//! Operator rebuild requests renew source authority and readiness before each bounded action.
use crate::{config::Project, index::ready};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::{RebuildAction, RebuildStatus, Store};
use std::path::Path;

/// All fields identify operator intent, never API-user visibility grants.
pub enum RebuildOperation {
    Start {
        namespace: String,
        name: String,
        revision: u32,
        previous: Option<u32>,
    },
    Step {
        limit: usize,
    },
    Status,
    Activate,
    Cancel,
}

/// A restarted CLI process reconstructs the same plan and checks current credentials, not a saved lease.
pub async fn rebuild(
    path: &Path,
    job: &str,
    operation: RebuildOperation,
) -> Result<RebuildStatus, String> {
    let project = Project::load(path)?;
    let mut auth =
        SourceAuthorization::new(project.authorization.checker().map_err(|e| e.to_string())?);
    auth.refresh_env(project.authorization.credential_env())
        .await
        .map_err(|e| e.to_string())?;
    ready(&project).await?;
    let job = job.to_owned();
    tokio::task::spawn_blocking(move || {
        let lease = auth.lease().ok_or("source lease unavailable")?;
        let mut store =
            Store::open_env(&project.destination_database_env).map_err(|e| e.to_string())?;
        match operation {
            RebuildOperation::Start {
                namespace,
                name,
                revision,
                previous,
            } => store.start_model_rebuild(
                &project.plan,
                lease,
                &job,
                &namespace,
                &name,
                revision,
                previous,
            ),
            other => {
                let (action, limit) = match other {
                    RebuildOperation::Step { limit } => (RebuildAction::Step, limit),
                    RebuildOperation::Status => (RebuildAction::Status, 32),
                    RebuildOperation::Activate => (RebuildAction::Activate, 32),
                    RebuildOperation::Cancel => (RebuildAction::Cancel, 32),
                    RebuildOperation::Start { .. } => unreachable!(),
                };
                store.model_rebuild(&project.plan, lease, &job, action, limit)
            }
        }
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "model rebuild worker failed")?
}
