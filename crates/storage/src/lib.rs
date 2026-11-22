//! Owned PostgreSQL state, lifecycle history, model projections and transactional recovery.
#![forbid(unsafe_code)]
mod error;
mod exercise;
mod journal;
mod lineage;
pub use lineage::{LineageDirection, LineageQuery};
mod live;
mod query;
pub use query::{PageKind, ReadPage};
mod migration;
mod model_registry;
pub use model_registry::ModelRegistration;
mod model_installation;
pub use model_installation::ModelInstallation;
mod model_projection;
pub use model_projection::{
    BoundModel, ModelBinding, ModelPage, RebuildAction, RebuildStatus, RelationshipQuery,
};
mod store;
pub use error::StoreError;
pub use migration::initialize_env;
pub use store::{Stats, Store};

#[cfg(test)]
mod tests;

mod selection_upgrade;
pub use selection_upgrade::{
    UpgradeBinding, UpgradeManifest, UpgradeResult, UpgradeStatus, upgrade_boundary,
};
