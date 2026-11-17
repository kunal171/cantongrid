//! Owned PostgreSQL persistence for the bounded reference ingestion engine.
#![forbid(unsafe_code)]
mod error;
mod migration;
pub use error::StoreError;
pub use migration::initialize_env;
