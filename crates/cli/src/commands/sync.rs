//! Offline schema synchronization and review bundle generation.
use cantongrid_runtime::config::read;
use std::path::Path;

const MAX_DAR_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MODEL_BYTES: u64 = 65536;

/// Dispatch offline schema sync actions; no source credentials or databases are required.
pub fn run(action: &str, args: &[String]) -> Result<(), String> {
    match (action, args) {
        ("plan", [old_dar, new_dar, models]) => {
            let plan = load(old_dar, new_dar, models)?;
            crate::print_json(
                &serde_json::to_value(&plan.report).map_err(|_| "cannot encode sync report")?,
            )
        }
        ("summary", [old_dar, new_dar, models]) => {
            let plan = load(old_dar, new_dar, models)?;
            print!("{}", plan.impact_summary());
            Ok(())
        }
        ("evaluate", [old_dar, new_dar, models, fixtures]) => {
            let plan = load(old_dar, new_dar, models)?;
            let bytes = read(Path::new(fixtures), MAX_MODEL_BYTES)
                .map_err(|_| "cannot read evaluation fixtures")?;
            let text = std::str::from_utf8(&bytes).map_err(|_| "fixtures must be UTF-8")?;
            let report = plan.evaluate(text).map_err(|e| e.to_string())?;
            crate::print_json(&serde_json::to_value(report).map_err(|_| "cannot encode evaluation report")?)
        }
        ("bundle", [old_dar, new_dar, models, out_dir]) => {
            let plan = load(old_dar, new_dar, models)?;
            let bundle = plan.bundle().map_err(|e| e.to_string())?;
            let out_path = Path::new(out_dir);
            std::fs::create_dir(out_path).map_err(|_| {
                "bundle output directory must be newly created; refusing to overwrite"
            })?;
            for (name, content) in &bundle.files {
                std::fs::write(out_path.join(name), content.as_bytes())
                    .map_err(|_| "cannot write bundle artifact")?;
            }
            crate::print_json(&serde_json::json!({
                "operation": "schema_sync_bundle",
                "output_directory": out_dir,
                "files_written": bundle.files.len(),
                "compatible": plan.report.compatible
            }))
        }
        _ => Err(
            "expected sync plan <old.dar> <new.dar> <models.json> or sync summary <old.dar> <new.dar> <models.json> or sync evaluate <old.dar> <new.dar> <models.json> <fixtures.json> or sync bundle <old.dar> <new.dar> <models.json> <output-directory>".into(),
        ),
    }
}

/// Share verified artifact loading across all read-only sync commands. Errors omit local paths.
fn load(old_dar: &str, new_dar: &str, models: &str) -> Result<cantongrid_sync::SyncPlan, String> {
    let old =
        read(Path::new(old_dar), MAX_DAR_BYTES).map_err(|_| "cannot read old DAR artifact")?;
    let new =
        read(Path::new(new_dar), MAX_DAR_BYTES).map_err(|_| "cannot read new DAR artifact")?;
    let bytes =
        read(Path::new(models), MAX_MODEL_BYTES).map_err(|_| "cannot read model configuration")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "model document must be UTF-8")?;
    cantongrid_sync::plan(&old, &new, text).map_err(|e| e.to_string())
}
