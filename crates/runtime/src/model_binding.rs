//! Operator activation uses the same verified runtime plan and current source checks as ingestion.
use crate::{config::Project, index::ready};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::{ModelBinding, Store};
use std::path::Path;

/// Backfill and bind one installed revision. Source permission/readiness failures occur before
/// destination writes; the short source lease is checked again inside the atomic storage commit.
pub async fn bind_model(
    path: &Path,
    namespace: &str,
    name: &str,
    revision: u32,
    previous: Option<u32>,
) -> Result<ModelBinding, String> {
    let project = Project::load(path)?;
    let mut authorization =
        SourceAuthorization::new(project.authorization.checker().map_err(|e| e.to_string())?);
    authorization
        .refresh_env(project.authorization.credential_env())
        .await
        .map_err(|e| e.to_string())?;
    ready(&project).await?;
    let namespace = namespace.to_owned();
    let name = name.to_owned();
    tokio::task::spawn_blocking(move || {
        let lease = authorization.lease().ok_or("source lease unavailable")?;
        let mut store =
            Store::open_env(&project.destination_database_env).map_err(|e| e.to_string())?;
        store
            .bind_model(&project.plan, lease, &namespace, &name, revision, previous)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "model binding worker failed")?
}
