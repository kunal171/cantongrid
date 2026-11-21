//! Atomic additive package handoff. A bounded baseline and model promotion share one commit;
//! the old checkpoint, contracts, events and batches are never reset or rewritten.
use crate::{Store, StoreError, journal::stream_key, live, model_projection as models};
use cantongrid_core::{
    authorization::SourceLease,
    config::Plan,
    lifecycle::{Action, Batch},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// A reviewed installed target, plus the exact prior binding expected at promotion.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeBinding {
    pub namespace: String,
    pub name: String,
    pub revision: u32,
    pub previous: Option<u32>,
    pub fingerprint: String,
}
/// Portable operator plan. This contains source scope metadata, never credentials or input paths.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeManifest {
    pub format_version: u32,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
    pub checkpoint: String,
    pub artifact_sha256: String,
    pub bindings: Vec<UpgradeBinding>,
}
impl UpgradeManifest {
    /// Bind idempotency to all reviewed inputs, including expected source progress and model revisions.
    pub fn fingerprint(&self) -> Result<String, StoreError> {
        let bytes = serde_json::to_vec(self).map_err(|_| StoreError::ModelDefinition)?;
        if bytes.len() > 65536 {
            return Err(StoreError::Limit);
        }
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect())
    }
}
/// A commit acknowledgement, not a claim of complete historical coverage for added templates.
#[derive(Debug, Serialize)]
pub struct UpgradeResult {
    pub changed: bool,
    pub baseline_contracts: usize,
    pub fingerprint: String,
}
/// Only append templates on the same physical source, party scope and generation.
/// Removing the templates key must leave byte-for-byte equivalent canonical definitions.
fn compatible(before: &Plan, after: &Plan) -> Result<(), StoreError> {
    let mut a = before.definition();
    let mut b = after.definition();
    a.as_object_mut()
        .ok_or(StoreError::PlanChanged)?
        .remove("templates");
    b.as_object_mut()
        .ok_or(StoreError::PlanChanged)?
        .remove("templates");
    if !before.is_live()
        || before.source_binding().is_none()
        || a != b
        || before.selected_template_count() >= after.selected_template_count()
        || before.templates().any(|t| !after.selects(t))
    {
        return Err(StoreError::PlanChanged);
    }
    Ok(())
}
/// Handoff only at a fully acknowledged interval boundary, never halfway through offset/PK ties.
pub fn upgrade_boundary(checkpoint: &str) -> Result<i64, StoreError> {
    let c: serde_json::Value =
        serde_json::from_str(checkpoint).map_err(|_| StoreError::InvalidBatch)?;
    let boundary = c["boundary"]
        .as_i64()
        .filter(|b| *b >= 0)
        .ok_or(StoreError::InvalidBatch)?;
    if c["snapshot"] != false
        || c["through"] != boundary
        || c["offset"] != boundary
        || c["pk"] != i64::MAX
    {
        return Err(StoreError::InvalidBatch);
    }
    Ok(boundary)
}
impl Store {
    /// Build a read-only reviewed plan from already registered/installed model revisions.
    /// No DDL, baseline insertion or source selection mutation occurs here.
    pub fn plan_selection_upgrade(
        &mut self,
        before: &Plan,
        after: &Plan,
        lease: &SourceLease,
        artifact_sha256: &str,
        targets: &[(String, String, u32)],
    ) -> Result<UpgradeManifest, StoreError> {
        compatible(before, after)?;
        if !models::authorized(before, lease) {
            return Err(StoreError::Authorization);
        }
        if targets.is_empty() || targets.len() > 16 || artifact_sha256.len() != 64 {
            return Err(StoreError::ModelDefinition);
        }
        let mut tx = self.client.transaction()?;
        models::lock(&mut tx, before)?;
        let (_, checkpoint) = live::prefix(&mut tx, before)?.ok_or(StoreError::ModelBinding)?;
        let checkpoint = checkpoint.ok_or(StoreError::ModelBinding)?;
        upgrade_boundary(&checkpoint)?;
        let key = stream_key(before).0;
        let mut bindings = Vec::new();
        let mut seen = BTreeSet::new();
        for (namespace, name, revision) in targets {
            if !seen.insert((namespace, name)) {
                return Err(StoreError::ModelDefinition);
            }
            let target = models::installed::load(&mut tx, namespace, name, *revision)?;
            if before.selects(&target.model.template) || !after.selects(&target.model.template) {
                return Err(StoreError::ModelBinding);
            }
            let previous = tx.query_opt("SELECT revision FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key,namespace,name])?
                .map(|r| u32::try_from(r.get::<_,i64>(0)).map_err(|_|StoreError::ModelCorrupt)).transpose()?;
            bindings.push(UpgradeBinding {
                namespace: namespace.clone(),
                name: name.clone(),
                revision: *revision,
                previous,
                fingerprint: target.sql.fingerprint,
            });
        }
        bindings.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
        let manifest = UpgradeManifest {
            format_version: 1,
            before: before.definition(),
            after: after.definition(),
            checkpoint,
            artifact_sha256: artifact_sha256.into(),
            bindings,
        };
        manifest.fingerprint()?;
        if !models::authorized(before, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(manifest)
    }
}

mod apply;
#[cfg(test)]
mod tests;
