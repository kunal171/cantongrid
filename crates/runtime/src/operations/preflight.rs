//! Read-only operational validation. A configured network label is not independent network evidence.
use super::*;
use cantongrid_ledger::transport::TransportPolicy;
use cantongrid_storage::Store;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Profiles enforce explicit transport and logical source separation before any network request.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkProfile {
    Localnet,
    Testnet,
    Mainnet,
}
impl NetworkProfile {
    /// Accept only the documented operator profile spellings, never a guessed hostname/network.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "localnet" => Ok(Self::Localnet),
            "testnet" => Ok(Self::Testnet),
            "mainnet" => Ok(Self::Mainnet),
            _ => Err("invalid network profile".into()),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Localnet => "localnet",
            Self::Testnet => "testnet",
            Self::Mainnet => "mainnet",
        }
    }
}
/// Validate profile selection and credential references without opening sockets or reading token values.
fn validate(project: &Project, profile: NetworkProfile) -> Result<(), String> {
    let (_, network, _) = project.plan.context().identity();
    let transport = if profile == NetworkProfile::Localnet {
        TransportPolicy::LocalnetHttp
    } else {
        TransportPolicy::RemoteHttps
    };
    if network != profile.name() || project.authorization.transport() != transport {
        return Err("configured network or transport does not match the requested profile".into());
    }
    if profile != NetworkProfile::Localnet && project.authorization.identity_settings().1.is_none()
    {
        return Err("remote profiles require an explicit expected token issuer".into());
    }
    Ok(())
}
/// Safe offline output reports configured policy only, never an actual remote/live acceptance claim.
fn configured(project: &Project, profile: NetworkProfile) -> serde_json::Value {
    serde_json::json!({"operation":"configuration_check","profile":profile,"configuredNetwork":profile.name(),"actualNetworkVerified":false,"participantPairing":"operator_configured_not_independently_proven","transport":project.authorization.transport(),"databaseTransport":"numeric_loopback_only_operator_tunnel_for_remote","sourceBinding":project.plan.source_binding(),"selectedTemplates":project.plan.selected_template_count(),"credentialValuesIncluded":false,"networkRequestsPerformed":false,"ledgerWrites":false})
}
/// Configuration-only validation still needs a nonsecret source database identity in its named environment variable.
pub fn check_configuration(
    path: &Path,
    profile: NetworkProfile,
) -> Result<serde_json::Value, String> {
    let project = Project::load(path)?;
    validate(&project, profile)?;
    Ok(configured(&project, profile))
}
/// Verify current Ledger rights, exporter health, restricted PQS schema/bounds and owned destination.
/// Read checks neither initialize a database nor upload/vet a DAR or submit a ledger transaction.
pub async fn preflight(path: &Path, profile: NetworkProfile) -> Result<serde_json::Value, String> {
    let project = Project::load(path)?;
    validate(&project, profile)?;
    crate::auth::validate_configuration(&project)?;
    let definition = project.plan.definition();
    let config = configured(&project, profile);
    let authorization = source(&project).await?;
    let report=tokio::task::spawn_blocking(move||{
        let lease=authorization.lease().ok_or("source authorization unavailable")?;
        let bounds=cantongrid_pqs::bounds_env(&project.source_database_env,&project.plan,lease).map_err(|_|"restricted source schema or bounds unavailable")?;
        Store::open_env(&project.destination_database_env).map_err(|_|"owned destination unavailable; initialize or review it explicitly")?;
        if lease.source_parties(&project.plan.context().binding(),std::time::Instant::now()).is_none(){return Err("source authorization expired".into());}
        Ok::<_,String>(serde_json::json!({"operation":"source_preflight","configured":config,"currentSourceRightsVerified":true,"exporterReady":true,"restrictedPqs":bounds,"ownedDestinationVerified":true,"destinationMigration":11,"ledgerWrites":false,"actualNetworkVerified":false,"liveAcceptanceComplete":false}))
    }).await.map_err(|_|"preflight worker unavailable")??;
    let current = Project::load(path)?;
    validate(&current, profile)?;
    if current.plan.definition() != definition {
        return Err("preflight plan changed".into());
    }
    crate::auth::validate_configuration(&current)?;
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
    Ok(report)
}
