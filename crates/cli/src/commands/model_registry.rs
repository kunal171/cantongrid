//! Explicit operator registry commands. No source credentials, schema installation or activation.
use cantongrid_models::Catalog;
use cantongrid_storage::Store;
use std::path::Path;

/// Persist a bounded catalog into an already initialized owned destination.
pub fn register(path: &Path, namespace: &str, variable: &str) -> Result<(), String> {
    let bytes = cantongrid_runtime::config::read(path, 65536)?;
    let catalog =
        Catalog::parse(std::str::from_utf8(&bytes).map_err(|_| "model document must be UTF-8")?)
            .map_err(|e| e.to_string())?;
    let mut store = Store::open_env(variable).map_err(|e| e.to_string())?;
    let result = store
        .register_models(namespace, &catalog)
        .map_err(|e| e.to_string())?;
    crate::print_json(&serde_json::json!({
        "operation":"model_registration","namespace":namespace,
        "catalogFingerprint":catalog.fingerprint(),"result":result,
        "activated":false
    }))
}

/// Report revision identity only; persisted literals are never printed into an operator summary.
pub fn inspect(namespace: &str, name: &str, revision: &str, variable: &str) -> Result<(), String> {
    let revision = parse_revision(revision)?;
    let mut store = Store::open_env(variable).map_err(|e| e.to_string())?;
    let model = store
        .registered_model(namespace, name, revision)
        .map_err(|e| e.to_string())?;
    let fingerprint = model
        .as_ref()
        .map(|model| model.fingerprint().map_err(|e| e.to_string()))
        .transpose()?;
    crate::print_json(&serde_json::json!({
        "operation":"model_revision_inspection","namespace":namespace,
        "model":name,"revision":revision,"registered":model.is_some(),
        "fingerprint":fingerprint
    }))
}

/// Registry and installation commands accept only the positive revision range used by the model.
pub(super) fn parse_revision(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| "model revision must be a positive u32".into())
}
