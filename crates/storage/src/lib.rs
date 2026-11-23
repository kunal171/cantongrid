//! Owned PostgreSQL state, lifecycle history, model projections and transactional recovery.
#![forbid(unsafe_code)]
mod delivery;
mod error;
mod operations;
pub use delivery::{
    Consumer, DeliveryPage, DeliveryQuery, DeliveryStart, Subscription, SubscriptionRequest,
};
mod exercise;
mod inspector;
mod journal;
mod lineage;
pub use lineage::{LineageDirection, LineageQuery};
mod live;
mod query;
mod temporal;
pub use query::{PageKind, ReadPage};
pub use temporal::TemporalQuery;
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

#[cfg(test)]
mod benchmark;
