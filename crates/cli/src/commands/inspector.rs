//! Read-only operator evidence: JSON summaries never embed supplied files or diagnostic logs.
use std::path::Path;

/// Accept exact command shapes before reading files or contacting any configured source.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    match (action, args) {
        ("evaluate", [definitions, name, payload]) => {
            let bytes = cantongrid_runtime::config::read(Path::new(definitions), 65536)?;
            let catalog = cantongrid_models::Catalog::parse(
                std::str::from_utf8(&bytes).map_err(|_| "invalid model document")?,
            )
            .map_err(|_| "invalid model document")?;
            let model = catalog
                .models()
                .iter()
                .find(|m| &m.name == name)
                .ok_or("model unavailable")?;
            let bytes = cantongrid_runtime::config::read(Path::new(payload), 65536)?;
            let payload = std::str::from_utf8(&bytes).map_err(|_| "invalid payload encoding")?;
            let finding = model
                .diagnose(payload)
                .map_err(|_| "model diagnostic unavailable")?;
            crate::print_json(
                &serde_json::json!({"operation":"inspector_evaluation","diagnostic":finding,"ledgerExecution":"not_performed"}),
            )
        }
        ("progress", [path]) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| "runtime unavailable")?;
            crate::print_json(
                &runtime.block_on(cantongrid_runtime::inspect_progress(Path::new(path)))?,
            )
        }
        _ => Err(
            "expected inspect evaluate <models> <name> <payload> or inspect progress <runtime>"
                .into(),
        ),
    }
}
