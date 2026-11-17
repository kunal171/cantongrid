//! Strict authorization-probe configuration, separate from fixture projects and credentials.
use crate::{GrantChecker, GrantError};
use cantongrid_core::{config::SourceContext, visibility::PartyId};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Credential values never belong in project files. Only the dedicated environment variable
/// name is accepted; the check reads its value when renewing authorization.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantCheckConfig {
    endpoint: String,
    user: String,
    source: String,
    network: String,
    generation: u64,
    parties: Vec<String>,
    credential_env: String,
}

impl GrantCheckConfig {
    /// Apply a byte bound before parsing and validate all fields before contacting a source.
    pub fn parse(document: &[u8]) -> Result<Self, GrantError> {
        if document.len() > 64 * 1024 {
            return Err(GrantError::Configuration);
        }
        let config: Self =
            serde_json::from_slice(document).map_err(|_| GrantError::Configuration)?;
        config.checker()?;
        if !crate::credential_variable(&config.credential_env) {
            return Err(GrantError::Configuration);
        }
        Ok(config)
    }

    /// Construct a checker tied to the explicit source/network/generation, not just a URL.
    pub fn checker(&self) -> Result<GrantChecker, GrantError> {
        let context =
            SourceContext::new(self.source.clone(), self.network.clone(), self.generation)
                .map_err(|_| GrantError::Configuration)?;
        let parties: BTreeSet<_> = self
            .parties
            .iter()
            .map(|party| PartyId::new(party.clone()))
            .collect::<Result<_, _>>()
            .map_err(|_| GrantError::Configuration)?;
        if parties.len() != self.parties.len() {
            return Err(GrantError::Configuration);
        }
        GrantChecker::localnet(&self.endpoint, &self.user, context.binding(), parties)
    }

    /// Configured identity and parties are selection inputs, not verified grants.
    pub fn context(&self) -> Result<SourceContext, GrantError> {
        SourceContext::new(self.source.clone(), self.network.clone(), self.generation)
            .map_err(|_| GrantError::Configuration)
    }
    pub fn parties(&self) -> Result<BTreeSet<PartyId>, GrantError> {
        self.parties
            .iter()
            .map(|p| PartyId::new(p.clone()).map_err(|_| GrantError::Configuration))
            .collect()
    }

    pub fn credential_env(&self) -> &str {
        &self.credential_env
    }
}
