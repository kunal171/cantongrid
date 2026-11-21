//! Atomic selection/baseline/model promotion under the existing canonical writer lock.
use super::*;
impl Store {
    /// Operator-only idempotency probe; apply still verifies the complete active definition under lock.
    pub fn selection_upgrade_committed(
        &mut self,
        after: &Plan,
        manifest: &UpgradeManifest,
    ) -> Result<bool, StoreError> {
        let key = stream_key(after).0;
        let fingerprint = manifest.fingerprint()?;
        Ok(self.client.query_opt("SELECT 1 FROM cantongrid.selection_upgrades WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fingerprint])?.is_some())
    }

    /// Replay a reviewed handoff safely after restart/uncertain commit. A stale plan fails before writes.
    /// Baseline admission is bounded to 64 records/1 MiB; a larger snapshot must not be truncated.
    pub fn apply_selection_upgrade(
        &mut self,
        before: &Plan,
        after: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
        baseline: &Batch,
    ) -> Result<UpgradeResult, StoreError> {
        compatible(before, after)?;
        if !models::authorized(before, lease) {
            return Err(StoreError::Authorization);
        }
        if manifest.format_version != 1
            || manifest.before != before.definition()
            || manifest.after != after.definition()
            || manifest.bindings.is_empty()
            || manifest.bindings.len() > 16
            || baseline.context != *before.context()
            || baseline.events.len() > 64
            || baseline
                .encode()
                .map_err(|_| StoreError::InvalidBatch)?
                .len()
                > 1024 * 1024
        {
            return Err(StoreError::InvalidBatch);
        }
        let boundary = upgrade_boundary(&manifest.checkpoint)?;
        if baseline.events.iter().any(|event| {
            before.selects(&event.template)
                || !after.selects(&event.template)
                || !matches!(event.action, Action::Create(_))
                || !event
                    .origin
                    .as_ref()
                    .is_some_and(|origin| origin.snapshot_observation && origin.offset <= boundary)
        }) {
            return Err(StoreError::InvalidBatch);
        }
        let fingerprint = manifest.fingerprint()?;
        let document = serde_json::to_value(manifest).map_err(|_| StoreError::InvalidBatch)?;
        let key = stream_key(before).0;
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        if let Some(row) = tx.query_opt("SELECT manifest FROM cantongrid.selection_upgrades WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fingerprint])? {
            if row.get::<_,serde_json::Value>(0) != document { return Err(StoreError::CorruptJournal); }
            live::prefix(&mut tx, after)?.ok_or(StoreError::CorruptJournal)?;
            if !models::authorized(after,lease) { return Err(StoreError::Authorization); }
            return Ok(UpgradeResult { changed:false, baseline_contracts:0, fingerprint });
        }
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled'))", &[&key])?.get::<_,bool>(0) { return Err(StoreError::Busy); }
        let (revision, checkpoint) =
            live::prefix(&mut tx, before)?.ok_or(StoreError::ModelBinding)?;
        if checkpoint.as_deref() != Some(&manifest.checkpoint) {
            return Err(StoreError::Busy);
        }
        if tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_rebuilds WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled'))", &[&key])?.get::<_,bool>(0) {
            return Err(StoreError::ModelRebuild);
        }
        // Verify every installed target and expected prior binding before changing any alias.
        let mut seen = BTreeSet::new();
        for binding in &manifest.bindings {
            if !seen.insert((&binding.namespace, &binding.name)) {
                return Err(StoreError::ModelBinding);
            }
            let target = models::installed::load(
                &mut tx,
                &binding.namespace,
                &binding.name,
                binding.revision,
            )?;
            if target.sql.fingerprint != binding.fingerprint
                || before.selects(&target.model.template)
                || !after.selects(&target.model.template)
            {
                return Err(StoreError::ModelBinding);
            }
            let previous = tx.query_opt("SELECT revision FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key,&binding.namespace,&binding.name])?.map(|r|r.get::<_,i64>(0));
            if previous != binding.previous.map(i64::from) {
                return Err(StoreError::ModelBinding);
            }
            let occupied: bool = tx
                .query_one(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM {} WHERE \"_cg_stream_key\"=$1)",
                        target.relation()
                    ),
                    &[&key],
                )?
                .get(0);
            if occupied {
                return Err(StoreError::ModelBinding);
            }
            tx.execute("INSERT INTO cantongrid.model_bindings(stream_key,namespace,name,revision,fingerprint,bound_at_revision) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(stream_key,namespace,name) DO UPDATE SET revision=EXCLUDED.revision,fingerprint=EXCLUDED.fingerprint,bound_at_revision=EXCLUDED.bound_at_revision,bound_at=now()", &[&key,&binding.namespace,&binding.name,&i64::from(binding.revision),&binding.fingerprint,&revision])?;
        }
        let bound = models::load_bound(&mut tx, after, revision)?;
        let mut baseline = baseline.clone();
        baseline.position = format!("selection-upgrade:{fingerprint}");
        live::persist_events(&mut tx, after, &baseline, &bound)?;
        tx.execute(
            "UPDATE cantongrid.streams SET definition=$2 WHERE stream_key=$1",
            &[&key, &after.definition()],
        )?;
        tx.execute("INSERT INTO cantongrid.selection_upgrades(stream_key,fingerprint,manifest) VALUES($1,$2,$3)", &[&key,&fingerprint,&document])?;
        if !models::authorized(after, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(UpgradeResult {
            changed: true,
            baseline_contracts: baseline.events.len(),
            fingerprint,
        })
    }
}
