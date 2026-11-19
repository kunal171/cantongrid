//! Independent organization caller authentication and scoped, integrity-protected page cursors.
use crate::config::{Project, read};
use cantongrid_core::visibility::{CallerGrants, PartyId};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

/// Reloadable organization policy; epoch/content changes invalidate previously issued cursors.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grants {
    version: u32,
    epoch: u64,
    principals: BTreeMap<String, Principal>,
}
/// Local access mapping for one bearer subject, independent from its Canton party identifiers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Principal {
    enabled: bool,
    parties: Vec<String>,
}
/// Organization bearer claims; signature, issuer, audience and bounded lifetime are checked separately.
#[derive(Deserialize, Serialize)]
struct Claims {
    sub: String,
    iss: String,
    aud: String,
    exp: u64,
    iat: u64,
}

/// Bound to a verified bearer and the exact current local grants/configuration fingerprint.
/// No source token, signing key or untrusted query-supplied party is retained here.
pub(crate) struct Identity {
    pub subject: String,
    pub expires: u64,
    pub binding: String,
    pub caller: CallerGrants,
}

/// Signed page position bound to caller, query and current policy; contents are not encrypted.
#[derive(Serialize, Deserialize)]
struct Cursor {
    sub: String,
    iss: String,
    aud: String,
    exp: u64,
    binding: String,
    kind: String,
    after: String,
}

/// Read Unix seconds for token expiry; an invalid clock fails authorization rather than extending it.
pub(crate) fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "authorization unavailable".into())
}

/// Keys are separate from one another and from the source credential. No development defaults.
fn keys(project: &Project) -> Result<(String, String), String> {
    let token =
        std::env::var(&project.api.token_key_env).map_err(|_| "authorization unavailable")?;
    let cursor =
        std::env::var(&project.api.cursor_key_env).map_err(|_| "authorization unavailable")?;
    let source = std::env::var(project.authorization.credential_env())
        .map_err(|_| "authorization unavailable")?;
    if token.len() < 32
        || cursor.len() < 32
        || token == cursor
        || token == source
        || cursor == source
    {
        return Err("authorization unavailable".into());
    }
    Ok((token, cursor))
}

/// Pin HS256, exact issuer/audience and zero expiry leeway for both bearer and cursor verification.
fn validation(project: &Project) -> Validation {
    let mut v = Validation::new(Algorithm::HS256);
    v.leeway = 0;
    v.validate_nbf = true;
    v.set_issuer(&[&project.api.issuer]);
    v.set_audience(&[&project.api.audience]);
    v.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
    v
}

/// Reload local grants for each request. Disabled/removed principals fail even with an unexpired
/// token; fingerprint changes invalidate all previous cursors before any SQL query.
pub(crate) fn authenticate(project: &Project, token: &str) -> Result<Identity, String> {
    if token.len() > 8192 {
        return Err("unauthorized".into());
    }
    let (key, _) = keys(project)?;
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(key.as_bytes()).map_err(|_| "authorization unavailable")?,
        &validation(project),
    )
    .map_err(|_| "unauthorized")?
    .claims;
    let time = now()?;
    // A valid signature does not permit future-issued or arbitrarily long-lived bearer claims.
    if claims.iat > time || claims.exp <= time || claims.exp.saturating_sub(claims.iat) > 3600 {
        return Err("unauthorized".into());
    }
    let bytes = read(&project.api.grants_file, 65536).map_err(|_| "authorization unavailable")?;
    let grants: Grants = cantongrid_core::document::parse(
        std::str::from_utf8(&bytes).map_err(|_| "authorization unavailable")?,
    )
    .map_err(|_| "authorization unavailable")?;
    if grants.version != 1 || grants.epoch == 0 || grants.principals.len() > 1024 {
        return Err("authorization unavailable".into());
    }
    let principal = grants
        .principals
        .get(&claims.sub)
        .filter(|p| p.enabled)
        .ok_or("unauthorized")?;
    let parties: BTreeSet<_> = principal
        .parties
        .iter()
        .map(|p| PartyId::new(p.clone()))
        .collect::<Result<_, _>>()
        .map_err(|_| "authorization unavailable")?;
    let source: BTreeSet<_> = project.plan.source_parties().cloned().collect();
    if parties.is_empty() || parties.len() != principal.parties.len() || !parties.is_subset(&source)
    {
        return Err("unauthorized".into());
    }
    // Fingerprint all authorization-relevant inputs without revealing party IDs in cursors.
    let binding_data = serde_json::json!([
        project.plan.definition(),
        grants.epoch,
        claims.sub,
        bytes,
        project.api.issuer,
        project.api.audience
    ]);
    let binding = Sha256::digest(binding_data.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(Identity {
        subject: claims.sub,
        expires: claims.exp,
        binding,
        caller: CallerGrants::from_verified(project.plan.context().binding(), parties),
    })
}

/// Sign a short-lived page position without exceeding the caller bearer lifetime or resume size limit.
pub(crate) fn cursor(
    project: &Project,
    id: &Identity,
    kind: &str,
    after: String,
) -> Result<String, String> {
    let (_, key) = keys(project)?;
    let claims = Cursor {
        sub: id.subject.clone(),
        iss: project.api.issuer.clone(),
        aud: project.api.audience.clone(),
        exp: (now()? + 60).min(id.expires),
        binding: id.binding.clone(),
        kind: kind.into(),
        after,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(key.as_bytes()).map_err(|_| "authorization unavailable")?,
    )
    .map_err(|_| "cursor unavailable")?;
    // JSON escaping and base64 can enlarge a model key; never issue a cursor we cannot resume.
    if token.len() > 8192 {
        return Err("cursor unavailable".into());
    }
    Ok(token)
}

/// A signed cursor is an opaque API token, not encrypted storage. Its key belongs only to
/// already-visible rows; principal, plan, grant epoch, query kind and expiry must all match.
pub(crate) fn after(
    project: &Project,
    id: &Identity,
    kind: &str,
    token: &str,
) -> Result<String, String> {
    if token.len() > 8192 {
        return Err("invalid cursor".into());
    }
    let (_, key) = keys(project)?;
    let cursor = decode::<Cursor>(
        token,
        &DecodingKey::from_secret(key.as_bytes()).map_err(|_| "authorization unavailable")?,
        &validation(project),
    )
    .map_err(|_| "invalid cursor")?
    .claims;
    if cursor.sub != id.subject
        || cursor.binding != id.binding
        || cursor.kind != kind
        || cursor.exp > id.expires
        || cursor.exp <= now()?
    {
        return Err("invalid cursor".into());
    }
    Ok(cursor.after)
}
