//! Explicit operator job commands. Validate the entire shape before opening a source connection.
use cantongrid_runtime::RebuildOperation;
use std::path::Path;

/// Parse one action without accepting extra flags or silently substituting activation for a step.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    let (path, job, operation) =
        match (action, args) {
            ("start", [path, job, namespace, name, revision, rest @ ..]) => {
                let revision = super::model_registry::parse_revision(revision)?;
                let previous = match rest {
                    [] => None,
                    [flag, value] if flag == "--replace" => {
                        Some(super::model_registry::parse_revision(value)?)
                    }
                    _ => return Err("expected optional --replace <previous-revision>".into()),
                };
                (
                    path,
                    job,
                    RebuildOperation::Start {
                        namespace: namespace.clone(),
                        name: name.clone(),
                        revision,
                        previous,
                    },
                )
            }
            ("step", [path, job, rest @ ..]) => {
                let limit = match rest {
                    [] => 32,
                    [flag, value] if flag == "--limit" => value
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=64).contains(n))
                        .ok_or("step limit must be 1..64")?,
                    _ => return Err("expected optional --limit <1..64>".into()),
                };
                (path, job, RebuildOperation::Step { limit })
            }
            ("status", [path, job]) => (path, job, RebuildOperation::Status),
            ("activate", [path, job]) => (path, job, RebuildOperation::Activate),
            ("cancel", [path, job]) => (path, job, RebuildOperation::Cancel),
            _ => return Err(
                "expected rebuild start|step|status|activate|cancel and exact positional arguments"
                    .into(),
            ),
        };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "could not initialize runtime")?;
    let result = runtime.block_on(cantongrid_runtime::rebuild(Path::new(path), job, operation))?;
    crate::print_json(&serde_json::json!({"operation":"model_rebuild","result":result}))
}
