//! SQL inspection is deliberately separate from source ingestion and destination migrations.

/// Print only operator metadata; the result never enables private lifecycle ingestion.
pub fn inspect(variable: &str) -> Result<(), String> {
    let report = cantongrid_pqs::inspect_env(variable).map_err(|e| e.to_string())?;
    crate::print_json(&serde_json::json!({"operation":"source_inspection","result":report}))
}
