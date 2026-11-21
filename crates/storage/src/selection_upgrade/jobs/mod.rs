//! Durable bounded snapshot staging. Public reads and ingestion pause only while publishing or
//! cancelling; private pages never become an alternate source of API-visible contracts.
use super::*;
use cantongrid_postgres::postgres::{GenericClient, Transaction};
use models::installed::Installed;

const MAX_STAGE_BYTES: i64 = 64 * 1024 * 1024;
const MAX_PAGES: i64 = 4096;

/// Operator progress survives process restarts; the cursor is source-internal, never an API cursor.
#[derive(Serialize)]
pub struct UpgradeStatus {
    pub phase: String,
    pub source_cursor: String,
    pub pages: i64,
    pub applied: i64,
    pub contracts: i64,
    pub retained_pages: i64,
}
/// Gate both canonical reads and writers during partial publication/undo. Query transactions see
/// either the preceding complete prefix or the new gate, never a partly published baseline.
pub(crate) fn available(client: &mut impl GenericClient, plan: &Plan) -> Result<(), StoreError> {
    if client.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND phase IN ('publishing','cancelling'))", &[&stream_key(plan).0])?.get::<_,bool>(0) {
        return Err(StoreError::Busy);
    }
    Ok(())
}
/// Lock/read only the exact manifest. A changed model/selection/anchor is a different job.
fn status(
    tx: &mut Transaction<'_>,
    key: &str,
    manifest: &UpgradeManifest,
) -> Result<UpgradeStatus, StoreError> {
    let fp = manifest.fingerprint()?;
    let row=tx.query_opt("SELECT manifest,phase,source_cursor,pages,applied,contracts,(SELECT count(*) FROM cantongrid.selection_upgrade_pages p WHERE p.stream_key=j.stream_key AND p.fingerprint=j.fingerprint) FROM cantongrid.selection_upgrade_jobs j WHERE stream_key=$1 AND fingerprint=$2 FOR UPDATE", &[&key,&fp])?.ok_or(StoreError::ModelBinding)?;
    if row.get::<_, serde_json::Value>(0)
        != serde_json::to_value(manifest).map_err(|_| StoreError::ModelDefinition)?
    {
        return Err(StoreError::CorruptJournal);
    }
    Ok(UpgradeStatus {
        phase: row.get(1),
        source_cursor: row.get(2),
        pages: row.get(3),
        applied: row.get(4),
        contracts: row.get(5),
        retained_pages: row.get(6),
    })
}
/// Verify the frozen anchor under the writer lock. No worker may move it during publication.
fn anchor(
    tx: &mut Transaction<'_>,
    before: &Plan,
    manifest: &UpgradeManifest,
) -> Result<i64, StoreError> {
    let (revision, checkpoint) =
        live::prefix_unblocked(tx, before)?.ok_or(StoreError::ModelBinding)?;
    if checkpoint.as_deref() != Some(&manifest.checkpoint) {
        return Err(StoreError::Busy);
    }
    Ok(revision)
}
/// Validate the complete reviewed contract before starting or resuming a state transition.
fn reviewed(before: &Plan, after: &Plan, manifest: &UpgradeManifest) -> Result<(), StoreError> {
    compatible(before, after)?;
    if manifest.format_version != 1
        || manifest.before != before.definition()
        || manifest.after != after.definition()
    {
        return Err(StoreError::PlanChanged);
    }
    manifest.fingerprint()?;
    Ok(())
}
/// Target tables must be installed, unreserved and outside the previous template selection.
/// Existing aliases may be replaced only if their current revision equals the reviewed previous value.
fn targets(
    tx: &mut Transaction<'_>,
    before: &Plan,
    after: &Plan,
    manifest: &UpgradeManifest,
    empty: bool,
) -> Result<Vec<Installed>, StoreError> {
    let key = stream_key(before).0;
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled'))", &[&key])?.get::<_,bool>(0) { return Err(StoreError::ModelRebuild); }
    let mut result = std::collections::BTreeMap::new();
    let mut aliases = BTreeSet::new();
    if manifest.bindings.is_empty() || manifest.bindings.len() > 16 {
        return Err(StoreError::ModelBinding);
    }
    for b in &manifest.bindings {
        if !aliases.insert((&b.namespace, &b.name)) {
            return Err(StoreError::ModelBinding);
        }
        let target = models::installed::load(tx, &b.namespace, &b.name, b.revision)?;
        if before.selects(&target.model.template)
            || !after.selects(&target.model.template)
            || target.sql.fingerprint != b.fingerprint
        {
            return Err(StoreError::ModelBinding);
        }
        let prior=tx.query_opt("SELECT revision FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key,&b.namespace,&b.name])?.map(|r|r.get::<_,i64>(0));
        if prior != b.previous.map(i64::from) {
            return Err(StoreError::ModelBinding);
        }
        if empty
            && tx
                .query_one(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM {} WHERE \"_cg_stream_key\"=$1)",
                        target.relation()
                    ),
                    &[&key],
                )?
                .get::<_, bool>(0)
        {
            return Err(StoreError::ModelBinding);
        }
        result.insert(target.sql.fingerprint.clone(), target);
    }
    let old: i64 = tx
        .query_one(
            "SELECT count(*) FROM cantongrid.model_bindings WHERE stream_key=$1",
            &[&key],
        )?
        .get(0);
    if old
        + manifest
            .bindings
            .iter()
            .filter(|b| b.previous.is_none())
            .count() as i64
        > 16
    {
        return Err(StoreError::Limit);
    }
    if tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled'))", &[&key])?.get::<_,bool>(0) { return Err(StoreError::ModelRebuild); }
    Ok(result.into_values().collect())
}
/// Recheck exact source scope immediately before every commit, including cancellation.
fn authority(plan: &Plan, lease: &SourceLease) -> Result<(), StoreError> {
    if !models::authorized(plan, lease) {
        return Err(StoreError::Authorization);
    }
    Ok(())
}

mod publication;
mod staging;
