//! Operator-pinned policy selection. Parsing this configuration never grants data access.
use cantongrid_models::reconciliation::Policy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path, sync::Arc};

/// Public configured identity; paths, literals and expression definitions are deliberately absent.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pin {
    pub name: String,
    pub namespace: String,
    pub model: String,
    pub model_revision: String,
    pub model_fingerprint: String,
    pub policy_name: String,
    pub policy_revision: String,
    pub policy_fingerprint: String,
}
/// Reusable pure policy plus one explicit active-model expectation; neither supplies caller grants.
#[derive(Clone)]
pub(crate) struct Entry {
    pub pin: Pin,
    pub policy: Arc<Policy>,
}
/// A bounded semantic catalog; compiled expressions stay private and cannot be deserialized as authority.
pub(crate) struct Catalog {
    pub entries: Vec<Entry>,
    pub fingerprint: String,
}
/// Strict version-1 manifest. Alias names permit one policy to be reused by different model bindings.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    policies: Vec<Definition>,
}
/// Only an operator's file can define paths and model pins; HTTP has no equivalent input type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    name: String,
    namespace: String,
    model: String,
    model_revision: u32,
    model_fingerprint: String,
    policy_file: std::path::PathBuf,
    policy_fingerprint: String,
}

/// Exact registry namespaces include hyphens; model/alias identifiers use the narrower compiler syntax.
fn identifier(name: &str, namespace: bool) -> bool {
    !name.is_empty()
        && name.len() <= if namespace { 64 } else { 48 }
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || (namespace && b == b'-'))
}
/// Require the canonical lowercase SHA-256 spelling instead of accepting ambiguous aliases.
fn fingerprint(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Catalog {
    /// Resolve bounded regular files relative to their manifest, without loading Project or any credential.
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let Some(path) = path else {
            return Ok(Self {
                entries: vec![],
                fingerprint: String::new(),
            });
        };
        let bytes = crate::config::read(path, 65536)?;
        let document: Document = cantongrid_core::document::parse(
            std::str::from_utf8(&bytes).map_err(|_| "invalid reconciliation encoding")?,
        )
        .map_err(|_| "invalid reconciliation configuration")?;
        if document.version != 1 || document.policies.is_empty() || document.policies.len() > 16 {
            return Err("invalid reconciliation catalog size or version".into());
        }
        let mut names = BTreeSet::new();
        let mut entries = Vec::new();
        for definition in document.policies {
            if !identifier(&definition.name, false)
                || !names.insert(definition.name.clone())
                || !identifier(&definition.namespace, true)
                || !identifier(&definition.model, false)
                || definition.model_revision == 0
                || !fingerprint(&definition.model_fingerprint)
                || !fingerprint(&definition.policy_fingerprint)
            {
                return Err("invalid reconciliation identity".into());
            }
            let policy_path = if definition.policy_file.is_relative() {
                path.parent()
                    .unwrap_or(Path::new("."))
                    .join(definition.policy_file)
            } else {
                definition.policy_file
            };
            let bytes = crate::config::read(&policy_path, 65536)?;
            let policy =
                Policy::parse(std::str::from_utf8(&bytes).map_err(|_| "invalid policy encoding")?)
                    .map_err(|_| "invalid configured policy")?;
            if policy.fingerprint() != definition.policy_fingerprint {
                return Err("configured policy fingerprint mismatch".into());
            }
            entries.push(Entry {
                pin: Pin {
                    name: definition.name,
                    namespace: definition.namespace,
                    model: definition.model,
                    model_revision: definition.model_revision.to_string(),
                    model_fingerprint: definition.model_fingerprint,
                    policy_name: policy.name().into(),
                    policy_revision: policy.revision().to_string(),
                    policy_fingerprint: definition.policy_fingerprint,
                },
                policy: Arc::new(policy),
            });
        }
        entries.sort_by(|a, b| a.pin.name.cmp(&b.pin.name));
        let canonical =
            serde_json::to_vec(&entries.iter().map(|entry| &entry.pin).collect::<Vec<_>>())
                .map_err(|_| "cannot encode reconciliation identity")?;
        let mut digest = Sha256::new();
        digest.update(b"cantongrid:reconciliation:catalog:v1\0");
        digest.update(canonical);
        Ok(Self {
            entries,
            fingerprint: digest
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests;
