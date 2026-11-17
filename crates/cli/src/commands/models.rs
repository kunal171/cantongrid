//! Offline model validation/planning/preview. These commands do not acquire source/API grants.
use cantongrid_models::Catalog;
use std::path::Path;

/// Make a definition concrete and reviewable without opening a source or mutating a database.
/// Preview explicitly reads an operator-supplied payload file; it is never a query fallback.
pub fn run(action: &str, path: &Path, rest: &[String]) -> Result<(), String> {
    if !matches!(
        (action, rest),
        ("validate" | "plan", []) | ("preview", [_, _])
    ) {
        return Err("unsupported model command arguments".into());
    }
    let bytes = cantongrid_runtime::config::read(path, 65536)?;
    let catalog =
        Catalog::parse(std::str::from_utf8(&bytes).map_err(|_| "model document must be UTF-8")?)
            .map_err(|e| e.to_string())?;
    match action {
        "validate" => crate::print_json(
            &serde_json::json!({"operation":"model_validation","models":catalog.models().len(),"fingerprint":catalog.fingerprint()}),
        ),
        "plan" => {
            let plans = catalog
                .models()
                .iter()
                .map(|model| model.sql_plan())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            crate::print_json(
                &serde_json::json!({"operation":"model_schema_plan","fingerprint":catalog.fingerprint(),"plans":plans,"applied":false}),
            )
        }
        "preview" => {
            let model = catalog
                .models()
                .iter()
                .find(|model| model.name == rest[0])
                .ok_or("unknown model selection")?;
            let bytes = cantongrid_runtime::config::read(Path::new(&rest[1]), 65536)?;
            let values = model
                .evaluate(std::str::from_utf8(&bytes).map_err(|_| "preview payload must be UTF-8")?)
                .map_err(|e| e.to_string())?;
            crate::print_json(
                &serde_json::json!({"operation":"model_preview","model":model.name,"revision":model.revision,"matched":values.is_some(),"values":values}),
            )
        }
        _ => Err("unsupported model command arguments".into()),
    }
}
