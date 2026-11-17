//! Read-only source inspection and current grants, separate from ingestion and migrations.

/// Print only operator metadata; the result never enables private lifecycle ingestion.
pub fn inspect(variable: &str) -> Result<(), String> {
    let report = cantongrid_pqs::inspect_env(variable).map_err(|e| e.to_string())?;
    crate::print_json(&serde_json::json!({"operation":"source_inspection","result":report}))
}

/// Probe current source grants without opening a destination or copying private contract data.
pub fn verify_grants(path: &str) -> Result<(), String> {
    use cantongrid_ledger::{SourceAuthorization, config::GrantCheckConfig};
    use std::io::Read;
    // Check before opening so a directory, device or FIFO cannot block this probe.
    if !std::fs::metadata(path)
        .map_err(|_| "cannot inspect source grant configuration")?
        .is_file()
    {
        return Err("source grant configuration must be a regular file".into());
    }
    let file = std::fs::File::open(path).map_err(|_| "cannot open source grant configuration")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect source grant configuration")?
        .is_file()
    {
        return Err("source grant configuration must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read source grant configuration")?;
    if bytes.len() > 65536 {
        return Err("source grant configuration exceeds 64 KiB".into());
    }
    let config = GrantCheckConfig::parse(&bytes).map_err(|e| e.to_string())?;
    let mut authorization = SourceAuthorization::new(config.checker().map_err(|e| e.to_string())?);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot initialize source authorization runtime")?;
    runtime
        .block_on(authorization.refresh_env(config.credential_env()))
        .map_err(|e| e.to_string())?;
    crate::print_json(&serde_json::json!({
        "operation": "source_grant_verification",
        "verified": true,
        "maximum_lease_seconds": 15,
        "history_coverage": "unknown",
        "private_lifecycle_ingestion_enabled": false
    }))
}
