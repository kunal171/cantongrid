//! A cursor state depends on permitted selected results, never a global hidden-update watermark.
use crate::StoreError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Internal key signed by the runtime's existing caller/policy/plan-bound cursor envelope.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    version: u8,
    state: String,
    contract: String,
}

/// Hash a bounded deterministic sequence of visible source values and sorted target value digests.
pub(super) fn state(
    source: &serde_json::Value,
    targets: &[(String, String)],
) -> Result<String, StoreError> {
    let bytes = serde_json::to_vec(&(source, targets)).map_err(|_| StoreError::ModelCorrupt)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
/// Reject a stale visible result before applying its keyset; never restart at an unfiltered page.
pub(super) fn after(text: Option<&str>, state: &str) -> Result<Option<String>, StoreError> {
    text.map(|text| {
        if text.len() > 8192 {
            return Err(StoreError::ModelQuery);
        }
        let key: Key =
            cantongrid_core::document::parse(text).map_err(|_| StoreError::ModelQuery)?;
        if key.version != 1
            || key.state != state
            || key.contract.is_empty()
            || key.contract.len() > 1024
            || key.contract.chars().any(char::is_control)
        {
            return Err(StoreError::ModelQuery);
        }
        Ok(key.contract)
    })
    .transpose()
}
/// Preserve exact contract IDs and a fixed-size result digest; neither is an authorization grant.
pub(super) fn encode(state: &str, contract: &str) -> Result<String, StoreError> {
    serde_json::to_string(&Key {
        version: 1,
        state: state.into(),
        contract: contract.into(),
    })
    .map_err(|_| StoreError::ModelCorrupt)
}
