//! Explicit source binding and revision promotion, separate from offline compilation/installation.
use std::path::Path;

/// Strict positional parsing prevents a misspelled replacement flag from changing a binding.
pub fn bind(
    path: &str,
    namespace: &str,
    name: &str,
    revision: &str,
    rest: &[String],
) -> Result<(), String> {
    let revision = super::model_registry::parse_revision(revision)?;
    let previous = match rest {
        [] => None,
        [flag, value] if flag == "--replace" => Some(super::model_registry::parse_revision(value)?),
        _ => return Err("expected optional --replace <previous-revision>".into()),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "could not initialize runtime")?;
    let result = runtime.block_on(cantongrid_runtime::bind_model(
        Path::new(path),
        namespace,
        name,
        revision,
        previous,
    ))?;
    crate::print_json(
        &serde_json::json!({"operation":"model_binding", "namespace":namespace, "model":name, "revision":revision, "result":result}),
    )
}
