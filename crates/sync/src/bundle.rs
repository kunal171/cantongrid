//! Generate complete review artifacts in memory. Emission applies no SQL and never changes
//! a processing expression, an installed revision or a live source checkpoint.
use crate::{SyncError as Error, SyncPlan};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Fixed filenames and hashes make a bundle portable and independently reviewable.
#[derive(Serialize)]
struct FileDigest {
    name: String,
    sha256: String,
    bytes: usize,
}
/// Caller-owned strings; the CLI writes these only into a newly created private directory.
pub struct Bundle {
    pub files: BTreeMap<String, String>,
}
impl SyncPlan {
    /// Generate only a fully compatible catalog. At Numeric scale changes, even an unchanged
    /// output SQL type is insufficient: comparison has already blocked the selected LF mismatch.
    pub fn bundle(&self) -> Result<Bundle, Error> {
        let catalog = self.catalog.as_ref().ok_or(Error::Incompatible)?;
        if !self.report.compatible {
            return Err(Error::Incompatible);
        }
        let mut files: BTreeMap<String, String> = BTreeMap::new();
        files.insert("metadata.json".into(), self.metadata.clone());
        files.insert(
            "models.json".into(),
            serde_json::to_string_pretty(
                &serde_json::json!({"version":1,"models":catalog.models()}),
            )
            .map_err(|_| Error::Definition)?,
        );
        files.insert(
            "schema.graphql".into(),
            catalog.graphql_schema().map_err(|_| Error::Definition)?,
        );
        files.insert(
            "types.ts".into(),
            catalog.typescript().map_err(|_| Error::Definition)?,
        );
        let mut sql = String::from(
            "-- Generated review plan. No SQL has been applied. Use models register/install.\n",
        );
        for model in catalog.models() {
            sql.push_str(&model.sql_plan().map_err(|_| Error::Definition)?.create_sql);
            sql.push('\n');
        }
        files.insert("schema.sql".into(), sql);
        files.insert(
            "compatibility.json".into(),
            serde_json::to_string_pretty(&self.report).map_err(|_| Error::Definition)?,
        );
        let before: BTreeSet<_> = self.previous.iter().map(|m| m.template.clone()).collect();
        let after: BTreeSet<_> = catalog
            .models()
            .iter()
            .map(|m| m.template.clone())
            .collect();
        files.insert("upgrade-input.json".into(),serde_json::to_string_pretty(&serde_json::json!({
            "format_version":1,"activation":if before==after {"no_activation_required"} else {"requires_live_upgrade_plan"},
            "old_artifact_sha256":self.report.old_artifact_sha256,"new_artifact_sha256":self.report.new_artifact_sha256,
            "old_models":self.previous.iter().map(|m|serde_json::json!({"name":m.name,"revision":m.revision})).collect::<Vec<_>>(),
            "new_models":catalog.models().iter().map(|m|serde_json::json!({"name":m.name,"revision":m.revision})).collect::<Vec<_>>(),
            "added_templates":after.difference(&before).collect::<Vec<_>>(),"retain_previous_templates":true
        })).map_err(|_|Error::Definition)?);
        // Metadata has its own 32 MiB bound; compiler/report artifacts share another 8 MiB.
        let auxiliary: usize = files
            .iter()
            .filter(|(name, _)| name.as_str() != "metadata.json")
            .map(|(_, v)| v.len())
            .sum();
        if auxiliary > 8 * 1024 * 1024 {
            return Err(Error::Limit);
        }
        let digests: Vec<_> = files
            .iter()
            .map(|(name, body)| FileDigest {
                name: name.clone(),
                bytes: body.len(),
                sha256: Sha256::digest(body.as_bytes())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            })
            .collect();
        files.insert(
            "bundle.json".into(),
            serde_json::to_string_pretty(&serde_json::json!({"format_version":1,"files":digests}))
                .map_err(|_| Error::Definition)?,
        );
        Ok(Bundle { files })
    }
}
