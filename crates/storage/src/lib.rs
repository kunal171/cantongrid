//! Owned PostgreSQL persistence for the bounded reference ingestion engine.
#![forbid(unsafe_code)]
mod error;
mod journal;
mod query;
pub use query::{PageKind, ReadPage};
mod migration;
mod store;
pub use error::StoreError;
pub use migration::initialize_env;
pub use store::{Stats, Store};

#[cfg(test)]
mod tests;
