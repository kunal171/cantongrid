//! Explicit source transport policy shared by Ledger checks and exporter readiness. Credentials
//! never belong in URLs; TLS verification and redirect rejection remain client-owned defaults.
use crate::GrantError;
use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Omitted policy preserves the existing numeric-loopback HTTP profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportPolicy {
    #[default]
    LocalnetHttp,
    RemoteHttps,
}
impl TransportPolicy {
    /// Validate the whole configured endpoint, not a substring or DNS resolution result.
    /// The exact path distinguishes a Ledger base URL from its separate exporter health URL.
    pub fn endpoint(self, value: &str, path: &str) -> Result<Url, GrantError> {
        if value.len() > 2048 {
            return Err(GrantError::Configuration);
        }
        let url = Url::parse(value).map_err(|_| GrantError::Configuration)?;
        if url.path() != path
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || match self {
                Self::LocalnetHttp => {
                    url.scheme() != "http"
                        || url.host_str() != Some("127.0.0.1")
                        || url.port().is_none()
                }
                Self::RemoteHttps => url.scheme() != "https",
            }
        {
            return Err(GrantError::Configuration);
        }
        Ok(url)
    }
    /// Disable proxies and every redirect, including same-host redirects, so bearer credentials
    /// can never be forwarded to a location selected by an HTTP response.
    pub fn client(self, timeout: Duration) -> Result<Client, GrantError> {
        Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .https_only(self == Self::RemoteHttps)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| GrantError::Configuration)
    }
}
