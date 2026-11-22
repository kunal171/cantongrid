//! Local operator operations require source credentials and separately owned destination access.
mod maintenance;
mod preflight;
use crate::{config::Project, index::ready};
use cantongrid_ledger::SourceAuthorization;
pub use maintenance::prune_deliveries;
pub use preflight::{NetworkProfile, check_configuration, preflight};

/// Share current source authorization and exporter readiness across the new operator commands.
async fn source(project: &Project) -> Result<SourceAuthorization, String> {
    let mut auth = SourceAuthorization::new(
        project
            .authorization
            .checker()
            .map_err(|_| "source authorization unavailable")?,
    );
    auth.refresh_env(project.authorization.credential_env())
        .await
        .map_err(|_| "source authorization unavailable")?;
    ready(project).await?;
    Ok(auth)
}
