//! Explicit schema installation. It never copies source data or activates processing.
use cantongrid_storage::Store;

/// Compile the registered revision and atomically install/verify its private PostgreSQL table.
pub fn install(namespace: &str, name: &str, revision: &str, variable: &str) -> Result<(), String> {
    let revision = super::model_registry::parse_revision(revision)?;
    let mut store = Store::open_env(variable).map_err(|e| e.to_string())?;
    let result = store
        .install_model(namespace, name, revision)
        .map_err(|e| e.to_string())?;
    crate::print_json(&serde_json::json!({
        "operation":"model_installation", "namespace":namespace, "model":name,
        "revision":revision, "result":result, "activated":false
    }))
}
