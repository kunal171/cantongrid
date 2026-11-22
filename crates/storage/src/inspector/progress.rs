//! Local operator progress only: no API caller receives global checkpoints or rebuild counters.
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan};
use cantongrid_postgres::postgres::IsolationLevel;
impl Store {
    /// Read immutable identities/counters without calling rebuild transitions or changing observations.
    pub fn inspect_progress(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
    ) -> Result<serde_json::Value, StoreError> {
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let (revision, checkpoint) =
            crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::InspectorUnavailable)?;
        let key = stream_key(plan).0;
        let cursor = checkpoint
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
        let boundary = cursor
            .as_ref()
            .and_then(|c| c["boundary"].as_i64())
            .map(|n| n.to_string());
        let bindings=tx.query("SELECT namespace,name,revision,fingerprint,bound_at_revision FROM cantongrid.model_bindings WHERE stream_key=$1 ORDER BY namespace,name LIMIT 17",&[&key])?;
        if bindings.len() > 16 {
            return Err(StoreError::Limit);
        }
        let models:Vec<_>=bindings.iter().map(|r|serde_json::json!({"namespace":r.get::<_,String>(0),"model":r.get::<_,String>(1),"revision":r.get::<_,i64>(2).to_string(),"fingerprint":r.get::<_,String>(3),"boundAtRevision":r.get::<_,i64>(4).to_string(),"maintenance":"atomic_with_canonical_lifecycle"})).collect();
        let jobs=tx.query("SELECT job_id,namespace,name,revision,phase,source_revision,scanned,processed_changes,projected_rows FROM cantongrid.model_rebuilds WHERE stream_key=$1 ORDER BY job_id LIMIT 65",&[&key])?;
        if jobs.len() > 64 {
            return Err(StoreError::Limit);
        }
        let rebuilds:Vec<_>=jobs.iter().map(|r|serde_json::json!({"job":r.get::<_,String>(0),"namespace":r.get::<_,String>(1),"model":r.get::<_,String>(2),"revision":r.get::<_,i64>(3).to_string(),"phase":r.get::<_,String>(4),"sourceRevision":r.get::<_,i64>(5).to_string(),"scanned":r.get::<_,i64>(6).to_string(),"processedChanges":r.get::<_,i64>(7).to_string(),"projectedRows":r.get::<_,i64>(8).to_string()})).collect();
        tx.commit()?;
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        Ok(
            serde_json::json!({"destinationRevision":revision.to_string(),"checkpointRecorded":checkpoint.is_some(),"destinationBoundary":boundary,"snapshot":cursor.as_ref().and_then(|c|c["snapshot"].as_bool()),"models":models,"rebuilds":rebuilds,"failedTrace":"not_recorded"}),
        )
    }
}
