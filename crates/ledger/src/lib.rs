//! Current source grants from the pinned Canton JSON Ledger API, separate from API callers.
//! Only explicitly configured loopback LocalNet endpoints are supported in this release.
#![forbid(unsafe_code)]

use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    visibility::{PartyId, ScopeBinding},
};
use reqwest::{Client, Url, header::HeaderValue, redirect::Policy};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    collections::BTreeSet,
    fmt,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_BODY: usize = 64 * 1024;
const MAX_PARTIES: usize = 128;
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

pub mod config;

/// Reserve a dedicated namespace; never read an unrelated variable such as HOME as a token.
fn credential_variable(variable: &str) -> bool {
    variable.starts_with("CANTONGRID_")
        && variable.len() > "CANTONGRID_".len()
        && variable.len() <= 128
        && variable
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Configured endpoint and exact read-only scope; this contains no source credential.
/// A party change requires an explicit scope-generation change, never silent widening.
pub struct GrantChecker {
    client: Client,
    endpoint: Url,
    user: String,
    scope: ScopeBinding,
    expected: BTreeSet<PartyId>,
}

/// Own the current lease so a failed or cancelled renewal cannot leave old success cached.
/// Consumers borrow this lease for reads; they cannot clone it or retain it across renewal.
pub struct SourceAuthorization {
    checker: GrantChecker,
    lease: Option<SourceLease>,
}

impl SourceAuthorization {
    pub fn new(checker: GrantChecker) -> Self {
        Self {
            checker,
            lease: None,
        }
    }

    /// Forget prior authorization before any fallible I/O, including environment lookup.
    pub async fn refresh_env(&mut self, variable: &str) -> Result<(), GrantError> {
        self.lease = None;
        self.lease = Some(self.checker.check_env(variable).await?);
        Ok(())
    }

    /// An available lease still requires its normal scope/expiry checks at read time.
    pub fn lease(&self) -> Option<&SourceLease> {
        self.lease.as_ref()
    }

    /// Invalidate immediately when the operator or source reports a known scope change.
    pub fn invalidate(&mut self) {
        self.lease = None;
    }
}

/// Source failures deliberately omit HTTP bodies, credentials, party IDs and URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantError {
    Configuration,
    Credential,
    Unavailable,
    Denied,
    Response,
    ScopeChanged,
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "invalid local Ledger grant-check configuration",
            Self::Credential => "source credential is missing, invalid or expired",
            Self::Unavailable => "current source authorization is unavailable",
            Self::Denied => "source authorization was denied",
            Self::Response => "unsupported source authorization response",
            Self::ScopeChanged => "source user or grants changed; explicit scope review required",
        })
    }
}
impl std::error::Error for GrantError {}

// Unverified claims are used ONLY to reject mismatched subjects/expired tokens and shorten
// leases. The configured Ledger API authenticates the actual bearer; claims never grant access.
#[derive(Deserialize)]
struct CredentialBounds {
    sub: String,
    exp: u64,
}

#[derive(Deserialize)]
struct UserResponse {
    user: User,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    id: String,
    #[serde(default)]
    is_deactivated: bool,
    #[serde(default)]
    identity_provider_id: String,
}
#[derive(Deserialize)]
struct RightsResponse {
    #[serde(default)]
    rights: Vec<Right>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Right {
    kind: Kind,
}
#[derive(Deserialize)]
enum Kind {
    CanReadAs(ReadAs),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadAs {
    value: Party,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Party {
    party: String,
}

impl GrantChecker {
    /// Construct a read-only checker. No DNS aliases, redirects, proxies or remote plaintext
    /// endpoints are permitted. TLS and non-default identity providers need separate review.
    pub fn localnet(
        endpoint: &str,
        user: &str,
        scope: ScopeBinding,
        parties: impl IntoIterator<Item = PartyId>,
    ) -> Result<Self, GrantError> {
        let endpoint = Url::parse(endpoint).map_err(|_| GrantError::Configuration)?;
        let expected: BTreeSet<_> = parties.into_iter().collect();
        if endpoint.scheme() != "http"
            || endpoint.host_str() != Some("127.0.0.1")
            || endpoint.port().is_none()
            || endpoint.path() != "/"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || user.is_empty()
            || user.len() > 128
            || !user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@^$.!`-#+'~_|:()".contains(&b))
            || expected.is_empty()
            || expected.len() > MAX_PARTIES
        {
            return Err(GrantError::Configuration);
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(CHECK_TIMEOUT)
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| GrantError::Configuration)?;
        Ok(Self {
            client,
            endpoint,
            user: user.to_owned(),
            scope,
            expected,
        })
    }

    /// Re-read the credential on every renewal. Errors yield no new lease; callers must discard
    /// an existing lease before attempting renewal, rather than serving on stale success.
    pub async fn check_env(&self, variable: &str) -> Result<SourceLease, GrantError> {
        if !credential_variable(variable) {
            return Err(GrantError::Configuration);
        }
        let token = std::env::var(variable).map_err(|_| GrantError::Credential)?;
        self.check(&token).await
    }

    /// Authenticate the source at the configured participant, bracket the exact rights read
    /// with user-state checks, and issue a lease measured from BEFORE the first HTTP request.
    /// The requests are not an atomic Ledger snapshot: revocation detection is bounded by the
    /// 15-second lease, and consumers must recheck that lease after database reads.
    pub async fn check(&self, token: &str) -> Result<SourceLease, GrantError> {
        let started = Instant::now();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GrantError::Credential)?;
        if token.len() > 8192 || token.is_empty() {
            return Err(GrantError::Credential);
        }
        let claims = jsonwebtoken::dangerous::insecure_decode::<CredentialBounds>(token)
            .map_err(|_| GrantError::Credential)?
            .claims;
        let remaining = Duration::from_secs(claims.exp)
            .checked_sub(wall)
            .filter(|ttl| !ttl.is_zero())
            .ok_or(GrantError::Credential)?;
        if claims.sub != self.user {
            return Err(GrantError::Credential);
        }
        let mut bearer = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| GrantError::Credential)?;
        bearer.set_sensitive(true);
        tokio::time::timeout(CHECK_TIMEOUT, async {
            self.check_user(&bearer).await?;
            let response: RightsResponse = self.get(true, &bearer).await?;
            let actual = read_parties(response)?;
            if actual != self.expected {
                return Err(GrantError::ScopeChanged);
            }
            self.check_user(&bearer).await?;
            // For example, a 2-second credential cannot produce a 15-second grant lease.
            let expires = started
                .checked_add(remaining.min(MAX_SOURCE_LEASE))
                .ok_or(GrantError::Credential)?;
            if Instant::now() >= expires {
                return Err(GrantError::Credential);
            }
            SourceLease::from_verified(
                self.scope.clone(),
                actual,
                started,
                MAX_SOURCE_LEASE,
                expires,
            )
            .map_err(|_| GrantError::Credential)
        })
        .await
        .map_err(|_| GrantError::Unavailable)?
    }

    /// Reject deactivated, substituted or non-default-identity-provider users.
    async fn check_user(&self, bearer: &HeaderValue) -> Result<(), GrantError> {
        let response: UserResponse = self.get(false, bearer).await?;
        if response.user.id != self.user
            || response.user.is_deactivated
            || !response.user.identity_provider_id.is_empty()
        {
            return Err(GrantError::ScopeChanged);
        }
        Ok(())
    }

    /// Perform GET only, with bounded streaming reads and redacted failures. A successful
    /// HTTP response alone is insufficient: its typed shape and exact grants must also pass.
    async fn get<T: DeserializeOwned>(
        &self,
        rights: bool,
        bearer: &HeaderValue,
    ) -> Result<T, GrantError> {
        let mut url = self.endpoint.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| GrantError::Configuration)?;
            path.clear().extend(["v2", "users", self.user.as_str()]);
            if rights {
                path.push("rights");
            }
        }
        let mut response = self
            .client
            .get(url)
            .header(reqwest::header::AUTHORIZATION, bearer.clone())
            .send()
            .await
            .map_err(|_| GrantError::Unavailable)?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 | 404 => return Err(GrantError::Denied),
            _ => return Err(GrantError::Unavailable),
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_BODY as u64)
        {
            return Err(GrantError::Response);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| GrantError::Unavailable)?
        {
            if chunk.len() > MAX_BODY - body.len() {
                return Err(GrantError::Response);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| GrantError::Response)
    }
}

/// Unknown rights (including ActAs, admin and wildcard reads) are rejected by deserialization;
/// duplicate grants are rejected rather than allowing an ambiguous source scope.
fn read_parties(response: RightsResponse) -> Result<BTreeSet<PartyId>, GrantError> {
    if response.rights.is_empty() || response.rights.len() > MAX_PARTIES {
        return Err(GrantError::ScopeChanged);
    }
    let mut parties = BTreeSet::new();
    for right in response.rights {
        let Kind::CanReadAs(read) = right.kind;
        let party = PartyId::new(read.value.party).map_err(|_| GrantError::Response)?;
        if !parties.insert(party) {
            return Err(GrantError::Response);
        }
    }
    Ok(parties)
}

#[cfg(test)]
mod tests;
