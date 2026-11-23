//! Offline diagnostics never load runtime configuration, read credentials or acquire source authority.
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
        _ => Err(
            "expected reconcile validate <policy.json> or reconcile evaluate <policy.json> <fixture.json>"
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
