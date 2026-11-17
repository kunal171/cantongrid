//! Atomic reference ingestion. Journal, projections, outbox and progress share one SQL commit.
use crate::{
    StoreError,
    journal::{MAX_BATCHES, MAX_JOURNAL_BYTES, recover, stream_key},
    migration::{check_destination, check_schema},
};
use cantongrid_core::{
    config::Plan,
    lifecycle::{Action, Batch, Engine},
};
use cantongrid_postgres::{
    connect_env,
    postgres::{Client, IsolationLevel, Transaction},
};
use std::collections::BTreeSet;

const MAX_BATCH_BYTES: usize = 1024 * 1024;
const MAX_EVENTS: usize = 1000;

/// A separately owned destination connection, never an upstream PQS migration client.
pub struct Store {
    pub(crate) client: Client,
}

impl Store {
    /// Open an explicitly initialized destination; opening never creates or changes schemas.
    pub fn open_env(variable: &str) -> Result<Self, StoreError> {
        let mut client = connect_env(variable)?;
        check_destination(&mut client)?;
        check_schema(&mut client)?;
        Ok(Self { client })
    }

    /// Publish one validated delivery. Replaying a committed delivery returns false without writes.
    pub fn apply(&mut self, plan: &Plan, batch: &Batch) -> Result<bool, StoreError> {
        self.apply_inner(plan, batch, false)
    }

    /// Stage all writes before one commit; the test-only fault exercises a real SQL transaction abort.
    pub(crate) fn apply_inner(
        &mut self,
        plan: &Plan,
        batch: &Batch,
        inject_failure: bool,
    ) -> Result<bool, StoreError> {
        let encoded = batch.encode().map_err(|_| StoreError::InvalidBatch)?;
        if encoded.len() > MAX_BATCH_BYTES || batch.events.len() > MAX_EVENTS {
            return Err(StoreError::Limit);
        }
        let (key, lock) = stream_key(plan);
        let mut tx = self.client.transaction()?;
        let acquired: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&lock])?
            .get(0);
        if !acquired {
            return Err(StoreError::Busy);
        }
        let (mut engine, revision) = recover(&mut tx, plan, &key)?;
        if !engine
            .apply(batch.clone())
            .map_err(|_| StoreError::InvalidBatch)?
        {
            return Ok(false);
        }
        if revision >= MAX_BATCHES {
            return Err(StoreError::Limit);
        }
        let bytes: i64 = tx.query_one("SELECT COALESCE(sum(octet_length(envelope::text)),0)::bigint FROM cantongrid.batches WHERE stream_key=$1", &[&key])?.get(0);
        // JSONB adds formatting whitespace, so measure its actual representation before publication.
        let document: serde_json::Value =
            serde_json::from_str(&encoded).map_err(|_| StoreError::InvalidBatch)?;
        let new_bytes: i32 = tx
            .query_one("SELECT octet_length($1::jsonb::text)", &[&document])?
            .get(0);
        if bytes + i64::from(new_bytes) > MAX_JOURNAL_BYTES {
            return Err(StoreError::Limit);
        }
        tx.execute("INSERT INTO cantongrid.streams(stream_key,definition) VALUES($1,$2) ON CONFLICT DO NOTHING", &[&key,&plan.definition()])?;
        let mut seen = BTreeSet::new();
        for event in &batch.events {
            if !plan.selects(&event.template) || !seen.insert(&event.id) {
                continue;
            }
            let witnesses: Vec<_> = event
                .witnesses
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect();
            let (kind, clauses) = match &event.action {
                Action::Create(payload) => {
                    let template = serde_json::to_value(&event.template)
                        .map_err(|_| StoreError::InvalidBatch)?;
                    let payload =
                        serde_json::to_value(payload).map_err(|_| StoreError::InvalidBatch)?;
                    tx.execute("INSERT INTO cantongrid.contracts(stream_key,contract_id,template,payload,create_event_id,witnesses,active) VALUES($1,$2,$3,$4,$5,$6,true)",
                        &[&key,&event.contract,&template,&payload,&event.id,&witnesses])?;
                    ("create", serde_json::json!([witnesses]))
                }
                Action::Archive | Action::ArchiveByPriorContract => {
                    let original: Vec<String> = tx.query_one("SELECT witnesses FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2 AND active", &[&key,&event.contract])?.get(0);
                    tx.execute("UPDATE cantongrid.contracts SET active=false,archive_event_id=$3 WHERE stream_key=$1 AND contract_id=$2", &[&key,&event.contract,&event.id])?;
                    // AND across clauses, OR inside each clause: never flatten independent evidence.
                    let clauses = if matches!(event.action, Action::ArchiveByPriorContract) {
                        serde_json::json!([original])
                    } else {
                        serde_json::json!([original, witnesses])
                    };
                    ("archive", clauses)
                }
            };
            let single = Batch {
                context: batch.context.clone(),
                position: batch.position.clone(),
                events: vec![event.clone()],
            };
            let body: serde_json::Value =
                serde_json::from_str(&single.encode().map_err(|_| StoreError::InvalidBatch)?)
                    .map_err(|_| StoreError::InvalidBatch)?;
            tx.execute("INSERT INTO cantongrid.events(stream_key,event_id,contract_id,position,kind,envelope,visibility_clauses) VALUES($1,$2,$3,$4,$5,$6,$7)",
                &[&key,&event.id,&event.contract,&batch.position,&kind,&body,&clauses])?;
            tx.execute(
                "INSERT INTO cantongrid.outbox(stream_key,event_id) VALUES($1,$2)",
                &[&key, &event.id],
            )?;
        }
        let next_revision = revision.checked_add(1).ok_or(StoreError::Limit)?;
        tx.execute("INSERT INTO cantongrid.batches(stream_key,revision,position,envelope) VALUES($1,$2,$3,$4)", &[&key,&next_revision,&batch.position,&document])?;
        tx.execute(
            "UPDATE cantongrid.streams SET revision=$2,checkpoint=$3 WHERE stream_key=$1",
            &[&key, &next_revision, &batch.position],
        )?;
        test_fault(&mut tx, inject_failure)?;
        // Failure to receive COMMIT's reply is ambiguous. A caller must reconnect and replay this batch.
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(true)
    }

    /// Recover an engine in one repeatable-read snapshot. Reads still require grants and a lease.
    pub fn restore(&mut self, plan: &Plan) -> Result<Engine, StoreError> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let engine = recover(&mut tx, plan, &stream_key(plan).0)?.0;
        tx.commit()?;
        Ok(engine)
    }

    /// Operator diagnostics only, not a caller-authorized API or an outbox cursor.
    pub fn stats(&mut self, plan: &Plan) -> Result<Stats, StoreError> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let (key, _) = stream_key(plan);
        let (_, revision) = recover(&mut tx, plan, &key)?;
        let row = tx.query_one("SELECT (SELECT count(*) FROM cantongrid.contracts WHERE stream_key=$1), (SELECT count(*) FROM cantongrid.contracts WHERE stream_key=$1 AND active), (SELECT count(*) FROM cantongrid.events WHERE stream_key=$1), (SELECT count(*) FROM cantongrid.outbox WHERE stream_key=$1)", &[&key])?;
        let stats = Stats {
            batches: revision,
            contracts: row.get(0),
            active_contracts: row.get(1),
            events: row.get(2),
            outbox: row.get(3),
        };
        tx.commit()?;
        Ok(stats)
    }
}

/// Counts for the operator who owns the destination database; never exposed as application counts.
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct Stats {
    pub batches: i64,
    pub contracts: i64,
    pub active_contracts: i64,
    pub events: i64,
    pub outbox: i64,
}

/// This branch is absent in production behavior; tests inject an actual database error before COMMIT.
fn test_fault(tx: &mut Transaction<'_>, inject: bool) -> Result<(), StoreError> {
    #[cfg(test)]
    if inject {
        tx.batch_execute("SELECT 1/0")?;
    }
    let _ = (tx, inject);
    Ok(())
}
