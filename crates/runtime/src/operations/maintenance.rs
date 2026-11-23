//! Explicit retention maintenance never uploads packages, submits ledger commands or rewrites history.
use super::*;
use cantongrid_storage::Store;
use std::path::Path;

/// Retain a bounded outbox tail and record consumer-specific loss under fresh source authority.
pub async fn prune_deliveries(
    path: &Path,
    retain: usize,
    limit: usize,
) -> Result<serde_json::Value, String> {
    if !(1..=100000).contains(&retain) || !(1..=256).contains(&limit) {
        return Err("invalid delivery retention budget".into());
    }
    let project = Project::load(path)?;
    let definition = project.plan.definition();
    let auth = source(&project).await?;
    let removed = tokio::task::spawn_blocking(move || {
        let lease = auth.lease().ok_or("source authorization unavailable")?;
        Store::open_env(&project.destination_database_env)
            .map_err(|_| "destination unavailable")?
            .prune_deliveries(&project.plan, lease, retain, limit)
            .map_err(|_| "delivery retention unavailable")
    })
    .await
    .map_err(|_| "retention worker unavailable")??;
    let latest = Project::load(path)?;
    if latest.plan.definition() != definition {
        return Err("retention plan changed".into());
    }
    let auth = source(&latest).await?;
    if auth
        .lease()
        .and_then(|lease| {
            lease.source_parties(&latest.plan.context().binding(), std::time::Instant::now())
        })
        .is_none()
    {
        return Err("source authorization expired".into());
    }
    Ok(
        serde_json::json!({"operation":"delivery_retention","retainedTarget":retain.to_string(),"batchLimit":limit,"removed":removed.to_string(),"checkpointChangesApplied":false,"canonicalHistoryDeleted":false,"consumerGapsMayBeRecorded":true}),
    )
}
