//! LocalNet reference runtime: source grants, owned persistence and authenticated query transports.
#![forbid(unsafe_code)]
pub mod config;
mod index;
pub use index::index;

mod api;
mod auth;
pub use api::serve;

mod model_binding;
pub use model_binding::bind_model;

mod model_rebuild;
pub use model_rebuild::{RebuildOperation, rebuild};

mod selection_upgrade;
pub use selection_upgrade::{UpgradeFiles, apply_upgrade, plan_upgrade};
