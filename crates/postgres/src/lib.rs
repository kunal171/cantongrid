//! Shared connection boundary for the LocalNet milestone. No source writes or migrations here.
#![forbid(unsafe_code)]
pub use postgres;
use postgres::{
    Client, Config, NoTls,
    config::{Host, SslMode},
};
use std::{fmt, time::Duration};

/// Read a dedicated operator environment variable without echoing its name/value on failure.
/// Generic process variables and implicit libpq credential fallback are not accepted.
pub fn connect_env(name: &str) -> Result<Client, DatabaseError> {
    if !name.starts_with("CANTONGRID_")
        || !name
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
    {
        return Err(DatabaseError::Configuration);
    }
    let value = std::env::var(name).map_err(|_| DatabaseError::Configuration)?;
    connect_local(&value)
}

/// Nonsecret target identity binds source-local offsets to a fixed participant database.
/// Password rotation does not change this identity; host/port/database/role changes do.
pub fn identity_env(name: &str) -> Result<String, DatabaseError> {
    if !name.starts_with("CANTONGRID_")
        || !name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(DatabaseError::Configuration);
    }
    identity_local(&std::env::var(name).map_err(|_| DatabaseError::Configuration)?)
}

pub fn identity_local(value: &str) -> Result<String, DatabaseError> {
    let config = local_config(value)?;
    let host = match &config.get_hosts()[0] {
        Host::Tcp(h) => h,
        _ => return Err(DatabaseError::Configuration),
    };
    Ok(format!(
        "{host}:{}:{:?}:{:?}",
        config.get_ports().first().unwrap_or(&5432),
        config.get_dbname(),
        config.get_user()
    ))
}

/// Local-only plaintext transport. Remote/TLS support needs its own verified configuration.
pub fn connect_local(value: &str) -> Result<Client, DatabaseError> {
    let config = local_config(value)?;
    config.connect(NoTls).map_err(|_| DatabaseError::Connection)
}

/// Validate before connecting, preventing DNS, multi-host and hostaddr escape from loopback.
fn local_config(value: &str) -> Result<Config, DatabaseError> {
    let mut config: Config = value.parse().map_err(|_| DatabaseError::Configuration)?;
    if !matches!(config.get_hosts(), [Host::Tcp(host)] if host == "127.0.0.1" || host == "::1")
        || config
            .get_hostaddrs()
            .iter()
            .any(|address| !address.is_loopback())
        || config.get_user().is_none()
        || config.get_dbname().is_none()
        || config.get_options().is_some()
        || config.get_ssl_mode() == SslMode::Require
    {
        return Err(DatabaseError::Configuration);
    }
    config.ssl_mode(SslMode::Disable).connect_timeout(Duration::from_secs(3))
        .options("-c statement_timeout=5000 -c lock_timeout=1000 -c idle_in_transaction_session_timeout=10000");
    Ok(config)
}

/// Never retain raw driver errors, SQL details, connection strings or bound payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseError {
    Configuration,
    Connection,
    Query,
}
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "invalid local database settings: require one numeric loopback host, explicit user/database, no options or required TLS",
            Self::Connection => "cannot connect to local database; check credentials and service health",
            Self::Query => "database operation failed; no state/progress acknowledgement was issued",
        })
    }
}
impl std::error::Error for DatabaseError {}
impl From<postgres::Error> for DatabaseError {
    fn from(_: postgres::Error) -> Self {
        Self::Query
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_remote_implicit_and_override_connections_without_network_calls() {
        for input in [
            "host=remote user=x dbname=x",
            "user=x dbname=x",
            "host=/tmp user=x dbname=x",
            "host=127.0.0.1,remote user=x dbname=x",
            "host=127.0.0.1 hostaddr=192.0.2.1 user=x dbname=x",
            "host=127.0.0.1 user=x dbname=x options=-csearch_path=public",
            "host=127.0.0.1 user=x dbname=x sslmode=require",
        ] {
            assert!(local_config(input).is_err());
        }
        assert!(local_config("host=127.0.0.1 user=x dbname=x password=synthetic").is_ok());
        assert!(local_config("host=::1 user=x dbname=x").is_ok());
        assert!(connect_env("HOME").is_err());
        let first =
            identity_local("host=127.0.0.1 port=5441 user=reader dbname=source password=first")
                .unwrap();
        assert_eq!(
            first,
            identity_local("host=127.0.0.1 port=5441 user=reader dbname=source password=rotated")
                .unwrap()
        );
        assert_ne!(
            first,
            identity_local("host=127.0.0.1 port=5442 user=reader dbname=source password=first")
                .unwrap()
        );
        assert!(!first.contains("password") && !first.contains("first"));
        assert!(!DatabaseError::Connection.to_string().contains("synthetic"));
    }
}
