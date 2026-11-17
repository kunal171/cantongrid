//! Bounded, ordered recovery from committed source batches; no trusted policies are stored.
use crate::StoreError;
use cantongrid_core::{
    config::Plan,
    lifecycle::{Batch, Engine},
};
use cantongrid_postgres::postgres::GenericClient;
use sha2::{Digest, Sha256};

pub(crate) const MAX_BATCHES: i64 = 256;
pub(crate) const MAX_JOURNAL_BYTES: i64 = 16 * 1024 * 1024;

/// A source context has one definition per generation; selection changes must not fork it silently.
pub(crate) fn stream_key(plan: &Plan) -> (String, i64) {
    let (source, network, generation) = plan.context().identity();
    let identity =
        serde_json::json!(["cantongrid-stream-v1", source, network, generation]).to_string();
    let hash = Sha256::digest(identity.as_bytes());
    let key = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    // A collision would deny concurrent work unnecessarily, never merge stream identities.
    let lock = i64::from_be_bytes(hash[..8].try_into().expect("SHA-256 has 32 bytes"));
    (key, lock)
}

/// Read under the caller's transaction snapshot, checking plan and checkpoint against the journal.
pub(crate) fn recover(
    client: &mut impl GenericClient,
    plan: &Plan,
    key: &str,
) -> Result<(Engine, i64), StoreError> {
    let mut engine = Engine::new(plan.clone());
    let row = client.query_opt(
        "SELECT definition,revision,checkpoint FROM cantongrid.streams WHERE stream_key=$1",
        &[&key],
    )?;
    let Some(row) = row else {
        return Ok((engine, 0));
    };
    let definition: serde_json::Value = row.get(0);
    if definition != plan.definition() {
        return Err(StoreError::PlanChanged);
    }
    let revision: i64 = row.get(1);
    let checkpoint: Option<String> = row.get(2);
    let size: i64 = client.query_one("SELECT COALESCE(sum(octet_length(envelope::text)),0)::bigint FROM cantongrid.batches WHERE stream_key=$1", &[&key])?.get(0);
    if size > MAX_JOURNAL_BYTES || !(0..=MAX_BATCHES).contains(&revision) {
        return Err(StoreError::Limit);
    }
    let rows = client.query("SELECT revision,position,envelope::text FROM cantongrid.batches WHERE stream_key=$1 ORDER BY revision LIMIT 257", &[&key])?;
    if rows.len() as i64 != revision {
        return Err(StoreError::CorruptJournal);
    }
    for (index, row) in rows.into_iter().enumerate() {
        let ordinal: i64 = row.get(0);
        let position: String = row.get(1);
        let encoded: String = row.get(2);
        let batch = Batch::decode(&encoded, plan).map_err(|_| StoreError::CorruptJournal)?;
        // Source positions remain opaque. Only the destination's contiguous revision orders replay.
        if ordinal != index as i64 + 1
            || position != batch.position
            || !engine
                .apply(batch)
                .map_err(|_| StoreError::CorruptJournal)?
        {
            return Err(StoreError::CorruptJournal);
        }
    }
    if engine.checkpoint() != checkpoint.as_deref() {
        return Err(StoreError::CorruptJournal);
    }
    Ok((engine, revision))
}
