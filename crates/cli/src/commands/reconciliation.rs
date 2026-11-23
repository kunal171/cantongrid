//! Offline diagnostics never load runtime configuration, read credentials or acquire source authority.
use cantongrid_models::Catalog;
use cantongrid_models::reconciliation::Policy;
use std::path::Path;

/// Accept only exact read-only command shapes before opening any explicitly supplied local files.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    match (action, args) {
        ("validate", [path]) => {
            let policy = load(path)?;
            crate::print_json(&serde_json::json!({
                "operation": "reconciliation_policy_validation",
                "policy": {
                    "name": policy.name(),
                    "revision": policy.revision().to_string(),
                    "fingerprint": policy.fingerprint()
                },
                "checks": policy.check_count(),
                "sourceAccess": "not_performed"
            }))
        }
        ("evaluate", [path, fixture]) => {
            let policy = load(path)?;
            let bytes = cantongrid_runtime::config::read(Path::new(fixture), 65536)?;
            let payload = std::str::from_utf8(&bytes).map_err(|_| "invalid fixture encoding")?;
            let report = policy.evaluate_fixture(payload);
            crate::print_json(&serde_json::json!({
                "operation": "reconciliation_fixture_evaluation",
                "report": report,
                "sourceAccess": "not_performed",
                "ledgerExecution": "not_performed"
            }))
        }
        ("preflight", [path, catalog, namespace, name]) => {
            let policy = load(path)?;
            let bytes = cantongrid_runtime::config::read(Path::new(catalog), 65536)?;
            let text = std::str::from_utf8(&bytes).map_err(|_| "invalid model catalog encoding")?;
            let catalog = Catalog::parse(text).map_err(|_| "invalid reconciliation model catalog")?;
            let model = catalog.models().iter().find(|model| model.name == *name)
                .ok_or("unknown reconciliation model selection")?;
            let preflight = policy.preflight_model(namespace, model)
                .map_err(|_| "incompatible reconciliation policy/model schema or namespace")?;
            crate::print_json(&serde_json::json!({
                "operation": "reconciliation_model_preflight",
                "preflight": preflight,
                "sourceAccess": "not_performed",
                "ledgerExecution": "not_performed"
            }))
        }
        _ => Err(
            "expected reconcile validate <policy.json>, reconcile evaluate <policy.json> <fixture.json> or reconcile preflight <policy.json> <catalog.json> <namespace> <model>"
                .into(),
        ),
    }
}

/// Keep source paths and parser diagnostics out of terminal/report failures; input is bounded and strict.
fn load(path: &str) -> Result<Policy, String> {
    let bytes = cantongrid_runtime::config::read(Path::new(path), 65536)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "invalid policy encoding")?;
    Policy::parse(text).map_err(|_| {
        "invalid reconciliation policy: check version, names, exact types and work limits".into()
    })
}
