//! Each worker cycle renews authorization before bounded SQL copying and atomic persistence.
use crate::config::Project;
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::Store;
use std::{path::Path, time::Duration};

/// Source readiness is independent from SQL reachability and user authorization.
/// Do not turn a failed/expired exporter into an apparently fresh cached source.
pub(crate) async fn ready(project: &Project) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|_| "source health client failed")?;
    let response = client
        .get(&project.source_health)
        .send()
        .await
        .map_err(|_| "source health unavailable")?;
    if !response.status().is_success() {
        return Err("source is not ready".into());
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "source health unavailable")?
    {
        if bytes.len() + chunk.len() > 4096 {
            return Err("invalid source health response".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let data: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "invalid source health response")?;
    if data["status"] != "ok"
        || ["grpc_up", "jdbc_connection_pool_up", "stream_up"]
            .iter()
            .any(|k| data[k] != true)
    {
        return Err("source is not ready".into());
    }
    Ok(())
}

/// `once` is a bounded page for scripts; the continuous loop retries without moving progress
/// on error. A source token is operator-managed and is re-read on each cycle.
pub async fn index(path: &Path, once: bool) -> Result<(), String> {
    loop {
        let result = cycle(path).await;
        if once {
            return result;
        }
        if result.is_err() {
            eprintln!(
                "CantonGrid: indexing paused; authorization, source or destination unavailable; checkpoint retained"
            );
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn cycle(path: &Path) -> Result<(), String> {
    let project = Project::load(path)?;
    let mut auth =
        SourceAuthorization::new(project.authorization.checker().map_err(|e| e.to_string())?);
    auth.refresh_env(project.authorization.credential_env())
        .await
        .map_err(|e| e.to_string())?;
    ready(&project).await?;
    // Blocking database work runs outside the async reactor; the lease travels with the job.
    tokio::task::spawn_blocking(move || {
        let lease = auth.lease().ok_or("source lease unavailable")?;
        let mut store =
            Store::open_env(&project.destination_database_env).map_err(|e| e.to_string())?;
        let checkpoint = store.checkpoint(&project.plan).map_err(|e| e.to_string())?;
        if let Some(batch) = cantongrid_pqs::page_env(
            &project.source_database_env,
            &project.plan,
            lease,
            checkpoint.as_deref(),
            project.page_size,
        )
        .map_err(|e| e.to_string())?
        {
            store
                .apply_page(&project.plan, &batch, lease, checkpoint.as_deref())
                .map_err(|e| e.to_string())?;
        } else {
            store
                .observe_authorized(&project.plan, lease)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|_| "index worker failed")?
}
