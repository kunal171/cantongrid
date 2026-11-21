//! Strict runtime configuration contains credential references, never secret values.
use cantongrid_core::config::{Plan, TemplateId};
use cantongrid_ledger::config::GrantCheckConfig;
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
};

/// Untrusted runtime wire configuration; credentials are environment references, not stored values.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    authorization: serde_json::Value,
    source_database_env: String,
    destination_database_env: String,
    source_health: String,
    templates: Vec<TemplateId>,
    history: String,
    archive_policy: String,
    payload: String,
    retention: String,
    api: Api,
    #[serde(default = "default_page_size")]
    page_size: usize,
}

/// Public transport settings and independent application identity/cursor key references.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Api {
    pub bind: std::net::SocketAddr,
    pub issuer: String,
    pub audience: String,
    pub token_key_env: String,
    pub cursor_key_env: String,
    pub grants_file: PathBuf,
}

/// One configuration binds the source checker, template selection, worker and API to one scope.
pub struct Project {
    pub authorization: GrantCheckConfig,
    pub plan: Plan,
    pub source_database_env: String,
    pub destination_database_env: String,
    pub source_health: String,
    pub api: Api,
    pub page_size: usize,
}

/// Read only bounded regular files. Paths and parser errors never flow into public errors.
pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    if !std::fs::metadata(path)
        .map_err(|_| "cannot inspect configuration")?
        .is_file()
    {
        return Err("configuration must be a regular file".into());
    }
    let file = std::fs::File::open(path).map_err(|_| "cannot open configuration")?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read configuration")?;
    if bytes.len() as u64 > limit {
        return Err("configuration exceeds size limit".into());
    }
    Ok(bytes)
}

/// The LocalNet milestone never guesses remote transport, retention or type policies.
fn default_page_size() -> usize {
    8
}

impl Project {
    /// Validate policies, scope and local endpoints before deriving the physical source plan.
    pub fn load(path: &Path) -> Result<Self, String> {
        let doc: Document = cantongrid_core::document::parse(
            std::str::from_utf8(&read(path, 65536)?)
                .map_err(|_| "invalid runtime configuration")?,
        )
        .map_err(|_| "invalid runtime configuration")?;
        if !(1..=8).contains(&doc.page_size)
            || doc.version != 1
            || doc.history != "snapshot_forward_partial"
            || doc.archive_policy != "prior_create_current_grants"
            || doc.payload != "raw_json"
            || doc.retention != "retain_while_authorized"
        {
            return Err("unsupported runtime policy".into());
        }
        let authorization = GrantCheckConfig::parse(
            &serde_json::to_vec(&doc.authorization)
                .map_err(|_| "invalid authorization configuration")?,
        )
        .map_err(|e| e.to_string())?;
        let count = doc.templates.len();
        let templates: BTreeSet<_> = doc.templates.into_iter().collect();
        if templates.len() != count {
            return Err("duplicate template selection".into());
        }
        let plan = Plan::pqs(
            authorization.context().map_err(|e| e.to_string())?,
            authorization.parties().map_err(|e| e.to_string())?,
            templates,
        )
        .map_err(|e| e.to_string())?;
        for variable in [
            &doc.source_database_env,
            &doc.destination_database_env,
            &doc.api.token_key_env,
            &doc.api.cursor_key_env,
        ] {
            if !variable.starts_with("CANTONGRID_")
                || variable.len() <= 11
                || !variable
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err("invalid credential reference".into());
            }
        }
        authorization
            .transport()
            .endpoint(&doc.source_health, "/readyz")
            .map_err(|_| "unsafe source health endpoint")?;
        if !doc.api.bind.ip().is_loopback()
            || doc.api.issuer.is_empty()
            || doc.api.audience.is_empty()
            || doc.source_database_env == doc.destination_database_env
            || doc.api.token_key_env == doc.api.cursor_key_env
        {
            return Err("unsafe runtime endpoint or key configuration".into());
        }
        let identity = cantongrid_postgres::identity_env(&doc.source_database_env)
            .map_err(|_| "source database identity unavailable")?;
        let plan = plan.with_source_binding(source_binding(
            authorization.endpoint(),
            authorization.user(),
            &identity,
            &doc.source_health,
        ));
        let plan = match authorization.identity_settings() {
            ("", None) => plan,
            (provider, issuer) => {
                use sha2::{Digest, Sha256};
                let identity = serde_json::json!([
                    "pqs041-source-auth-v2",
                    plan.source_binding(),
                    provider,
                    issuer
                ]);
                plan.with_source_binding(
                    Sha256::digest(identity.to_string().as_bytes())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect(),
                )
            }
        };
        let mut api = doc.api;
        if api.grants_file.is_relative() {
            api.grants_file = path
                .parent()
                .unwrap_or(Path::new("."))
                .join(&api.grants_file);
        }
        Ok(Self {
            authorization,
            plan,
            source_database_env: doc.source_database_env,
            destination_database_env: doc.destination_database_env,
            source_health: doc.source_health,
            api,
            page_size: doc.page_size,
        })
    }
}

/// Stable, nonsecret source-instance fingerprint shared by operator fixtures and the runtime.
pub fn source_binding(endpoint: &str, user: &str, database: &str, health: &str) -> String {
    use sha2::{Digest, Sha256};
    let value = serde_json::json!(["pqs041-template-v1", endpoint, user, database, health]);
    Sha256::digest(value.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
