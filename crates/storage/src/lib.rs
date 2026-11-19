//! Owned PostgreSQL state, lifecycle history, model projections and transactional recovery.
#![forbid(unsafe_code)]
mod error;
mod journal;
mod live;
mod query;
pub use query::{PageKind, ReadPage};
mod migration;
mod model_registry;
pub use model_registry::ModelRegistration;
mod model_installation;
pub use model_installation::ModelInstallation;
mod model_projection;
pub use model_projection::{BoundModel, ModelBinding, ModelPage, RebuildAction, RebuildStatus};
mod store;
pub use error::StoreError;
pub use migration::initialize_env;
pub use store::{Stats, Store};

#[cfg(test)]
mod tests;
