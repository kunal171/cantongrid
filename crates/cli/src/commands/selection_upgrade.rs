//! Strict positional parsing for read-only planning versus explicit owned-destination application.
use cantongrid_runtime::{UpgradeFiles, apply_upgrade, plan_upgrade};
use std::path::Path;

/// Build one short-lived operator runtime; no credentials are accepted as command-line arguments.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "could not initialize runtime")?;
    match (action,args) {
        ("plan", [before,after,dar,models,namespace]) => {
            let files = UpgradeFiles { before:Path::new(before),after:Path::new(after),dar:Path::new(dar),models:Path::new(models) };
            crate::print_json(&serde_json::json!(runtime.block_on(plan_upgrade(files,namespace))?))
        }
        ("apply", [manifest,before,after,dar,models]) => {
            let files = UpgradeFiles { before:Path::new(before),after:Path::new(after),dar:Path::new(dar),models:Path::new(models) };
            crate::print_json(&serde_json::json!(runtime.block_on(apply_upgrade(files,Path::new(manifest)))?))
        }
        _ => Err("expected upgrade plan <old-runtime> <new-runtime> <dar> <models> <namespace> or upgrade apply <manifest> <old-runtime> <new-runtime> <dar> <models>".into()),
    }
}
