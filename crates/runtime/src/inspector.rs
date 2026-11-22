//! Local operator observations use current source grants; public API callers cannot invoke this path.
use crate::{config::Project, index::ready};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::Store;
use std::path::Path;

/// Renew source authority/health for a bounded read, never mutate worker or rebuild progress.
pub async fn inspect_progress(path: &Path) -> Result<serde_json::Value, String> {
    let project = Project::load(path)?;
    let definition = project.plan.definition();
    let mut auth = SourceAuthorization::new(
        project
            .authorization
            .checker()
            .map_err(|_| "source authorization unavailable")?,
    );
    auth.refresh_env(project.authorization.credential_env())
        .await
        .map_err(|_| "source authorization unavailable")?;
    ready(&project).await?;
    let report=tokio::task::spawn_blocking(move||{
        let lease=auth.lease().ok_or("source authorization unavailable")?;
        let source=cantongrid_pqs::bounds_env(&project.source_database_env,&project.plan,lease).map_err(|_|"source bounds unavailable")?;
        let destination=Store::open_env(&project.destination_database_env).map_err(|_|"destination unavailable")?.inspect_progress(&project.plan,lease).map_err(|_|"destination progress unavailable")?;
        let latest=source["latest"].as_str().and_then(|n|n.parse::<i64>().ok());
        let indexed=destination["destinationBoundary"].as_str().and_then(|n|n.parse::<i64>().ok());
        // Example: source 100 versus indexed 90 means 10 source-local offsets, not 10 events/seconds.
        let distance=latest.zip(indexed).and_then(|(a,b)|a.checked_sub(b)).map(|v|v.to_string());
        Ok::<_,String>(serde_json::json!({"operation":"inspector_progress","source":source,"destination":destination,"boundaryDifference":distance,"differenceUnit":"source_local_offsets_not_events_or_time","observationsAtomic":false,"historyComplete":false}))
    }).await.map_err(|_|"inspector worker unavailable")??;
    let latest = Project::load(path)?;
    if latest.plan.definition() != definition {
        return Err("inspector plan changed".into());
    }
    let mut current = SourceAuthorization::new(
        latest
            .authorization
            .checker()
            .map_err(|_| "source authorization unavailable")?,
    );
    current
        .refresh_env(latest.authorization.credential_env())
        .await
        .map_err(|_| "source authorization unavailable")?;
    ready(&latest).await?;
    if current
        .lease()
        .and_then(|l| l.source_parties(&latest.plan.context().binding(), std::time::Instant::now()))
        .is_none()
    {
        return Err("source authorization expired".into());
    }
    Ok(report)
}
