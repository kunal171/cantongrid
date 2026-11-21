//! Page publication and cancellation retain the public gate until a complete prefix is restored.
use super::*;
impl Store {
    /// Publish or undo one stored page. Each page commits independently while the public gate
    /// stays closed; the final promotion makes the complete widened prefix visible atomically.
    pub fn step_selection_upgrade(
        &mut self,
        before: &Plan,
        after: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
    ) -> Result<UpgradeStatus, StoreError> {
        reviewed(before, after, manifest)?;
        authority(before, lease)?;
        let key = stream_key(before).0;
        let fp = manifest.fingerprint()?;
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        let state = status(&mut tx, &key, manifest)?;
        match state.phase.as_str() {
            "publishing" => {
                let revision = anchor(&mut tx, before, manifest)?;
                let installed = targets(&mut tx, before, after, manifest, false)?;
                let row=tx.query_opt("SELECT envelope::text FROM cantongrid.selection_upgrade_pages WHERE stream_key=$1 AND fingerprint=$2 AND page=$3", &[&key,&fp,&(state.applied+1)])?.ok_or(StoreError::CorruptJournal)?;
                let mut batch = Batch::decode(&row.get::<_, String>(0), after)
                    .map_err(|_| StoreError::InvalidBatch)?;
                batch.position = format!("selection-upgrade:{fp}:{}", state.applied + 1);
                live::persist_events(&mut tx, after, &batch, &installed)?;
                tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET applied=applied+1 WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
                if state.applied + 1 == state.pages {
                    for b in &manifest.bindings {
                        tx.execute("INSERT INTO cantongrid.model_bindings(stream_key,namespace,name,revision,fingerprint,bound_at_revision) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(stream_key,namespace,name) DO UPDATE SET revision=EXCLUDED.revision,fingerprint=EXCLUDED.fingerprint,bound_at_revision=EXCLUDED.bound_at_revision,bound_at=now()", &[&key,&b.namespace,&b.name,&i64::from(b.revision),&b.fingerprint,&revision])?;
                    }
                    models::load_bound(&mut tx, after, revision)?;
                    tx.execute(
                        "UPDATE cantongrid.streams SET definition=$2 WHERE stream_key=$1",
                        &[&key, &after.definition()],
                    )?;
                    tx.execute("INSERT INTO cantongrid.selection_upgrades(stream_key,fingerprint,manifest) VALUES($1,$2,$3)", &[&key,&fp,&serde_json::to_value(manifest).map_err(|_|StoreError::ModelDefinition)?])?;
                    tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='complete' WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
                }
            }
            "cancelling" | "complete" => {
                let cleanup:i64=tx.query_one("SELECT cleanup_after FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?.get(0);
                if cleanup < state.pages {
                    let row=tx.query_opt("SELECT envelope::text FROM cantongrid.selection_upgrade_pages WHERE stream_key=$1 AND fingerprint=$2 AND page=$3", &[&key,&fp,&(cleanup+1)])?.ok_or(StoreError::CorruptJournal)?;
                    if state.phase == "cancelling" && cleanup < state.applied {
                        anchor(&mut tx, before, manifest)?;
                        let installed = targets(&mut tx, before, after, manifest, false)?;
                        let batch = Batch::decode(&row.get::<_, String>(0), after)
                            .map_err(|_| StoreError::InvalidBatch)?;
                        for event in batch.events {
                            // Only unpublished baseline rows are eligible. Forward ingestion is gated,
                            // so no legitimate archive or subsequent contract version can be removed.
                            for target in &installed {
                                tx.execute(&format!("DELETE FROM {} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2",target.relation()), &[&key,&event.contract])?;
                            }
                            tx.execute(
                                "DELETE FROM cantongrid.outbox WHERE stream_key=$1 AND event_id=$2",
                                &[&key, &event.id],
                            )?;
                            tx.execute(
                                "DELETE FROM cantongrid.events WHERE stream_key=$1 AND event_id=$2",
                                &[&key, &event.id],
                            )?;
                            let removed=tx.execute("DELETE FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2 AND create_event_id=$3 AND active", &[&key,&event.contract,&event.id])?;
                            if removed != 1 {
                                return Err(StoreError::CorruptJournal);
                            }
                        }
                    }
                    tx.execute("DELETE FROM cantongrid.selection_upgrade_pages WHERE stream_key=$1 AND fingerprint=$2 AND page=$3", &[&key,&fp,&(cleanup+1)])?;
                    tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET cleanup_after=cleanup_after+1 WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
                    if state.phase == "cancelling" && cleanup + 1 == state.pages {
                        tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='cancelled' WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
                    }
                } else if state.phase == "cancelling" {
                    tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='cancelled' WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
                }
            }
            "cancelled" => {}
            _ => return Err(StoreError::ModelBinding),
        }
        let state = status(&mut tx, &key, manifest)?;
        authority(before, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
    /// Cancellation before completion restores the old public prefix through bounded cleanup.
    /// Completed handoffs use model rebuild rollback and retain new-package history instead.
    pub fn cancel_selection_upgrade(
        &mut self,
        before: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
    ) -> Result<UpgradeStatus, StoreError> {
        authority(before, lease)?;
        let key = stream_key(before).0;
        let fp = manifest.fingerprint()?;
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        let state = status(&mut tx, &key, manifest)?;
        if !["snapshot", "ready", "publishing", "cancelling", "cancelled"]
            .contains(&state.phase.as_str())
        {
            return Err(StoreError::ModelBinding);
        }
        if state.phase != "cancelled" {
            tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='cancelling' WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
        }
        let state = status(&mut tx, &key, manifest)?;
        authority(before, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
}
