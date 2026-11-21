//! Persist source snapshot pages privately before freezing the reviewed canonical prefix.
use super::*;
impl Store {
    /// Reserve a durable private staging job; repeating start never resets its cursor or pages.
    pub fn start_selection_upgrade(
        &mut self,
        before: &Plan,
        after: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
    ) -> Result<UpgradeStatus, StoreError> {
        reviewed(before, after, manifest)?;
        authority(before, lease)?;
        if manifest.format_version != 1
            || manifest.before != before.definition()
            || manifest.after != after.definition()
        {
            return Err(StoreError::ModelDefinition);
        }
        let boundary = upgrade_boundary(&manifest.checkpoint)?;
        let key = stream_key(before).0;
        let fp = manifest.fingerprint()?;
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        if tx.query_opt("SELECT 1 FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?.is_none() {
            anchor(&mut tx,before,manifest)?; targets(&mut tx,before,after,manifest,true)?;
            let active:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND phase NOT IN ('complete','cancelled'))", &[&key])?.get(0);
            if active { return Err(StoreError::Busy); }
            let cursor=serde_json::json!({"snapshot":true,"boundary":boundary,"through":boundary,"offset":boundary,"pk":0}).to_string();
            tx.execute("INSERT INTO cantongrid.selection_upgrade_jobs(stream_key,fingerprint,manifest,phase,source_cursor) VALUES($1,$2,$3,'snapshot',$4)", &[&key,&fp,&serde_json::to_value(manifest).map_err(|_|StoreError::ModelDefinition)?,&cursor])?;
        }
        let prior = status(&mut tx, &key, manifest)?;
        if prior.phase == "cancelled" {
            if prior.retained_pages != 0 {
                return Err(StoreError::CorruptJournal);
            }
            anchor(&mut tx, before, manifest)?;
            targets(&mut tx, before, after, manifest, true)?;
            let boundary = upgrade_boundary(&manifest.checkpoint)?;
            let cursor=serde_json::json!({"snapshot":true,"boundary":boundary,"through":boundary,"offset":boundary,"pk":0}).to_string();
            tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='snapshot',source_cursor=$3,pages=0,applied=0,contracts=0,staged_bytes=0,cleanup_after=0 WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp,&cursor])?;
        }
        let state = status(&mut tx, &key, manifest)?;
        authority(before, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
    /// Read exact operator progress. This does not expose contract contents or grant API access.
    pub fn selection_upgrade_status(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
    ) -> Result<UpgradeStatus, StoreError> {
        authority(plan, lease)?;
        let mut tx = self.client.transaction()?;
        let state = status(&mut tx, &stream_key(plan).0, manifest)?;
        authority(plan, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
    /// Persist one source page against its expected cursor; private page data is bounded and not served.
    pub fn stage_selection_upgrade(
        &mut self,
        before: &Plan,
        lease: &SourceLease,
        manifest: &UpgradeManifest,
        expected: &str,
        batch: &Batch,
    ) -> Result<UpgradeStatus, StoreError> {
        authority(before, lease)?;
        let fp = manifest.fingerprint()?;
        let key = stream_key(before).0;
        let encoded = batch.encode().map_err(|_| StoreError::InvalidBatch)?;
        let cursor: serde_json::Value =
            serde_json::from_str(&batch.position).map_err(|_| StoreError::InvalidBatch)?;
        let boundary = upgrade_boundary(&manifest.checkpoint)?;
        if batch.context != *before.context()
            || batch.events.len() > 64
            || encoded.len() > 1024 * 1024
            || cursor["boundary"] != boundary
            || cursor["through"] != boundary
            || !cursor["snapshot"].is_boolean()
            || batch.events.iter().any(|e| {
                before.selects(&e.template)
                    || !manifest.after["templates"].as_array().is_some_and(|a| {
                        a.contains(&serde_json::to_value(&e.template).unwrap_or_default())
                    })
                    || !matches!(e.action, Action::Create(_))
                    || !e.origin.as_ref().is_some_and(|o| {
                        o.snapshot_observation && o.offset >= 0 && o.offset <= boundary
                    })
            })
        {
            return Err(StoreError::InvalidBatch);
        }
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        anchor(&mut tx, before, manifest)?;
        let state = status(&mut tx, &key, manifest)?;
        if state.phase != "snapshot" || state.source_cursor != expected {
            return Err(StoreError::Busy);
        }
        let size:i64=tx.query_one("SELECT staged_bytes FROM cantongrid.selection_upgrade_jobs WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?.get(0);
        if state.pages >= MAX_PAGES || size + encoded.len() as i64 > MAX_STAGE_BYTES {
            return Err(StoreError::Limit);
        }
        if cursor["snapshot"] == false {
            upgrade_boundary(&batch.position)?;
        }
        tx.execute("INSERT INTO cantongrid.selection_upgrade_pages(stream_key,fingerprint,page,envelope) VALUES($1,$2,$3,$4)", &[&key,&fp,&(state.pages+1),&serde_json::from_str::<serde_json::Value>(&encoded).map_err(|_|StoreError::InvalidBatch)?])?;
        tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET pages=pages+1,contracts=contracts+$3,staged_bytes=staged_bytes+$4,source_cursor=$5,phase=$6 WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp,&(batch.events.len() as i64),&(encoded.len() as i64),&batch.position,&if cursor["snapshot"]==false {"ready"} else {"snapshot"}])?;
        let state = status(&mut tx, &key, manifest)?;
        authority(before, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
    /// Freeze the reviewed prefix and gate API reads/writes before any canonical baseline is published.
    pub fn activate_selection_upgrade(
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
        if state.phase == "ready" {
            anchor(&mut tx, before, manifest)?;
            targets(&mut tx, before, after, manifest, true)?;
            tx.execute("UPDATE cantongrid.selection_upgrade_jobs SET phase='publishing' WHERE stream_key=$1 AND fingerprint=$2", &[&key,&fp])?;
        } else if !["publishing", "complete"].contains(&state.phase.as_str()) {
            return Err(StoreError::ModelBinding);
        }
        let state = status(&mut tx, &key, manifest)?;
        authority(before, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(state)
    }
}
