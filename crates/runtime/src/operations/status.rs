//! Local operator status combines separately observed progress and durable queue pressure.
use super::*;
use cantongrid_storage::Store;
use std::path::Path;

/// Require both current source authority and owned destination access; this is not a public metrics route.
pub async fn status(path: &Path) -> Result<serde_json::Value, String> {
    let progress = crate::inspect_progress(path).await?;
    let project = Project::load(path)?;
    let definition = project.plan.definition();
    let authorization = source(&project).await?;
    let delivery = tokio::task::spawn_blocking(move || {
        let lease = authorization
            .lease()
            .ok_or("source authorization unavailable")?;
        Store::open_env(&project.destination_database_env)
            .map_err(|_| "owned destination unavailable")?
            .operational_metrics(&project.plan, lease)
            .map_err(|_| "operator metrics unavailable")
    })
    .await
    .map_err(|_| "operator metrics worker unavailable")??;
    let current = Project::load(path)?;
    if current.plan.definition() != definition {
        return Err("operator status plan changed".into());
    }
    let authorization = source(&current).await?;
    if authorization
        .lease()
        .and_then(|l| {
            l.source_parties(&current.plan.context().binding(), std::time::Instant::now())
        })
        .is_none()
    {
        return Err("source authorization expired".into());
    }
    Ok(
        serde_json::json!({"operation":"operator_status","progress":progress,"delivery":delivery,"resourceLimits":crate::api::resource_limits(),"observationsAtomic":false,"liveAcceptanceComplete":false,"credentialValuesIncluded":false}),
    )
}
