//! Durable rebuilds reserve inactive physical rows; only a guarded activation makes them queryable.
mod step;
mod transition;
use super::{MAX_BINDINGS, authorized, installed, load_bound, lock};
use crate::{Store, StoreError, journal::stream_key, model_registry};
use cantongrid_core::{authorization::SourceLease, config::Plan};
use cantongrid_postgres::postgres::{Row, Transaction};
use serde::Serialize;

/// Operator actions never authenticate API callers or grant access to staged rows.
#[derive(Clone, Copy)]
pub enum RebuildAction {
    Step,
    Status,
    Activate,
    Cancel,
}

/// Progress contains counters only; private scan keys and contract payloads stay in storage.
/// projected_rows is a build counter, not a live count after activation.
#[derive(Debug, Serialize)]
pub struct RebuildStatus {
    pub job: String,
    pub phase: String,
    pub scanned: String,
    pub processed_changes: String,
    pub projected_rows: String,
    pub source_revision: String,
    pub has_pending: bool,
}

/// Identity and progress loaded under the same source lock used by ingestion and activation.
struct Job {
    id: String,
    namespace: String,
    name: String,
    revision: u32,
    fingerprint: String,
    previous: Option<u32>,
    previous_fingerprint: Option<String>,
    phase: String,
    after: Option<String>,
    source_revision: i64,
    scanned: i64,
    processed: i64,
    projected: i64,
}
impl Job {
    /// Serialize exact counters without exposing the private scan cursor.
    fn status(&self, pending: bool) -> RebuildStatus {
        RebuildStatus {
            job: self.id.clone(),
            phase: self.phase.clone(),
            scanned: self.scanned.to_string(),
            processed_changes: self.processed.to_string(),
            projected_rows: self.projected.to_string(),
            source_revision: self.source_revision.to_string(),
            has_pending: pending,
        }
    }
    /// Persist progress in the transaction that wrote or removed the corresponding rows.
    fn save(&self, tx: &mut Transaction<'_>, key: &str) -> Result<(), StoreError> {
        tx.execute("UPDATE cantongrid.model_rebuilds SET phase=$3,scan_after=$4,source_revision=$5,scanned=$6,processed_changes=$7,projected_rows=$8,updated_at=now() WHERE stream_key=$1 AND job_id=$2", &[&key,&self.id,&self.phase,&self.after,&self.source_revision,&self.scanned,&self.processed,&self.projected])?;
        Ok(())
    }
}

/// All operations require the exact physical plan and a newly checked, unexpired source lease.
fn scope(plan: &Plan, lease: &SourceLease, job: &str) -> Result<String, StoreError> {
    if !authorized(plan, lease) {
        return Err(StoreError::Authorization);
    }
    if !plan.is_live()
        || !plan.source_binding().is_some_and(|b| {
            b.len() == 64
                && b.bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
    {
        return Err(StoreError::ModelBinding);
    }
    if job.is_empty()
        || job.len() > 64
        || !job
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(StoreError::ModelRebuild);
    }
    Ok(stream_key(plan).0)
}
/// Reject malformed persisted progress before it can drive writes or activation.
fn decode(row: Row) -> Result<Job, StoreError> {
    let revision = |value: i64| {
        u32::try_from(value)
            .ok()
            .filter(|v| *v > 0)
            .ok_or(StoreError::ModelRebuild)
    };
    let job = Job {
        id: row.get(0),
        namespace: row.get(1),
        name: row.get(2),
        revision: revision(row.get(3))?,
        fingerprint: row.get(4),
        previous: row.get::<_, Option<i64>>(5).map(revision).transpose()?,
        previous_fingerprint: row.get(6),
        phase: row.get(7),
        after: row.get(8),
        source_revision: row.get(9),
        scanned: row.get(10),
        processed: row.get(11),
        projected: row.get(12),
    };
    if job.source_revision < 1
        || job.scanned < 0
        || job.processed < 0
        || job.projected < 0
        || job.previous.is_some() != job.previous_fingerprint.is_some()
        || ![
            "cleaning",
            "backfill",
            "catchup",
            "ready",
            "cleanup",
            "complete",
            "cancelling",
            "cancelled",
        ]
        .contains(&job.phase.as_str())
    {
        return Err(StoreError::ModelRebuild);
    }
    Ok(job)
}
/// Lock job metadata after acquiring the canonical source writer lock.
fn read(tx: &mut Transaction<'_>, key: &str, id: &str) -> Result<Job, StoreError> {
    decode(tx.query_opt("SELECT job_id,namespace,name,revision,fingerprint,previous_revision,previous_fingerprint,phase,scan_after,source_revision,scanned,processed_changes,projected_rows FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND job_id=$2 FOR UPDATE", &[&key,&id])?.ok_or(StoreError::ModelRebuild)?)
}
/// Readiness is invalidated by newly queued changes, even if the phase still says ready.
fn pending(tx: &mut Transaction<'_>, key: &str, id: &str) -> Result<bool, StoreError> {
    Ok(tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_rebuild_dirty WHERE stream_key=$1 AND job_id=$2)", &[&key,&id])?.get(0))
}

/// Reservations protect inactive target rows and retired rows awaiting bounded cleanup.
/// Called under the source lock before any legacy bind or new rebuild may reuse a table.
pub(super) fn reserved(
    tx: &mut Transaction<'_>,
    key: &str,
    namespace: &str,
    name: &str,
    fp: &str,
) -> Result<bool, StoreError> {
    Ok(tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled') AND ((namespace=$2 AND name=$3) OR fingerprint=$4 OR (phase='cleanup' AND previous_fingerprint=$4)))", &[&key,&namespace,&name,&fp])?.get(0))
}

/// Coalesce canonical changes atomically. A failing staged expression does not stop the live model.
pub(crate) fn changed(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    contract: &str,
    template: &cantongrid_core::config::TemplateId,
) -> Result<(), StoreError> {
    let template = serde_json::to_value(template).map_err(|_| StoreError::ModelRebuild)?;
    tx.execute("INSERT INTO cantongrid.model_rebuild_dirty(stream_key,job_id,contract_id) SELECT stream_key,job_id,$2 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND template=$3 AND phase IN ('cleaning','backfill','catchup','ready') ON CONFLICT DO NOTHING", &[&stream_key(plan).0,&contract,&template])?;
    Ok(())
}

impl Store {
    /// Start is idempotent for the same operator job/options. One source permits at most 16 live jobs.
    #[allow(clippy::too_many_arguments)]
    pub fn start_model_rebuild(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        id: &str,
        namespace: &str,
        name: &str,
        revision: u32,
        previous: Option<u32>,
    ) -> Result<RebuildStatus, StoreError> {
        model_registry::validate_key(namespace, name, revision)?;
        let key = scope(plan, lease, id)?;
        let mut tx = self.client.transaction()?;
        lock(&mut tx, plan)?;
        let source_revision = crate::live::prefix(&mut tx, plan)?
            .ok_or(StoreError::ModelBinding)?
            .0;
        let target = installed::load(&mut tx, namespace, name, revision)?;
        if !plan.selects(&target.model.template) {
            return Err(StoreError::ModelBinding);
        }
        if tx
            .query_opt(
                "SELECT 1 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND job_id=$2",
                &[&key, &id],
            )?
            .is_some()
        {
            let job = read(&mut tx, &key, id)?;
            if job.namespace != namespace
                || job.name != name
                || job.revision != revision
                || job.previous != previous
                || job.fingerprint != target.sql.fingerprint
            {
                return Err(StoreError::ModelRebuild);
            }
            let result = job.status(pending(&mut tx, &key, id)?);
            if !authorized(plan, lease) {
                return Err(StoreError::Authorization);
            }
            tx.commit()?;
            return Ok(result);
        }
        if reserved(&mut tx, &key, namespace, name, &target.sql.fingerprint)? {
            return Err(StoreError::ModelRebuild);
        }
        let active = load_bound(&mut tx, plan, source_revision)?;
        if active
            .iter()
            .any(|model| model.sql.fingerprint == target.sql.fingerprint)
        {
            return Err(StoreError::ModelRebuild);
        }
        let old=tx.query_opt("SELECT revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key,&namespace,&name])?;
        let actual = old.as_ref().map(|r| r.get::<_, i64>(0));
        if actual != previous.map(i64::from) {
            return Err(StoreError::ModelRebuild);
        }
        let count:i64=tx.query_one("SELECT count(*) FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled')", &[&key])?.get(0);
        if count >= MAX_BINDINGS as i64
            || (old.is_none()
                && tx
                    .query_one(
                        "SELECT count(*) FROM cantongrid.model_bindings WHERE stream_key=$1",
                        &[&key],
                    )?
                    .get::<_, i64>(0)
                    >= MAX_BINDINGS as i64)
        {
            return Err(StoreError::Limit);
        }
        let old_fp = old.map(|row| row.get::<_, String>(1));
        let template =
            serde_json::to_value(&target.model.template).map_err(|_| StoreError::ModelRebuild)?;
        tx.execute("INSERT INTO cantongrid.model_rebuilds(stream_key,job_id,namespace,name,revision,fingerprint,template,previous_revision,previous_fingerprint,phase,source_revision) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'cleaning',$10)", &[&key,&id,&namespace,&name,&i64::from(revision),&target.sql.fingerprint,&template,&previous.map(i64::from),&old_fp,&source_revision])?;
        let result = read(&mut tx, &key, id)?.status(false);
        if !authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(result)
    }

    /// A page and its durable progress share a commit; activation never copies an unbounded table.
    pub fn model_rebuild(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        id: &str,
        action: RebuildAction,
        limit: usize,
    ) -> Result<RebuildStatus, StoreError> {
        self.model_rebuild_checked(plan, lease, id, action, limit, |_| Ok(()))
    }

    /// Fault injection proves that page rows/progress roll back and leases are checked after writes.
    #[allow(clippy::too_many_arguments)]
    fn model_rebuild_checked(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        id: &str,
        action: RebuildAction,
        limit: usize,
        before_commit: impl FnOnce(&mut Transaction<'_>) -> Result<(), StoreError>,
    ) -> Result<RebuildStatus, StoreError> {
        if !(1..=64).contains(&limit) {
            return Err(StoreError::Limit);
        }
        let key = scope(plan, lease, id)?;
        let mut tx = self.client.transaction()?;
        lock(&mut tx, plan)?;
        let revision = crate::live::prefix(&mut tx, plan)?
            .ok_or(StoreError::ModelBinding)?
            .0;
        let mut job = read(&mut tx, &key, id)?;
        let target = installed::load(&mut tx, &job.namespace, &job.name, job.revision)?;
        if target.sql.fingerprint != job.fingerprint || !plan.selects(&target.model.template) {
            return Err(StoreError::ModelRebuild);
        }
        let template: serde_json::Value = tx
            .query_one(
                "SELECT template FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND job_id=$2",
                &[&key, &id],
            )?
            .get(0);
        if template
            != serde_json::to_value(&target.model.template).map_err(|_| StoreError::ModelRebuild)?
        {
            return Err(StoreError::ModelRebuild);
        }
        match action {
            RebuildAction::Status => {}
            RebuildAction::Step => step::run(&mut tx, plan, &key, &mut job, &target, limit as i64)?,
            RebuildAction::Activate => {
                transition::activate(&mut tx, plan, &key, &mut job, revision)?
            }
            RebuildAction::Cancel => transition::cancel(&mut job)?,
        }
        if !matches!(action, RebuildAction::Status) {
            job.source_revision = revision;
            job.save(&mut tx, &key)?;
        }
        let result = job.status(pending(&mut tx, &key, id)?);
        before_commit(&mut tx)?;
        if !authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
