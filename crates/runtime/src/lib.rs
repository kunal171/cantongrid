//! LocalNet reference runtime: source grants, owned persistence and authenticated query transports.
#![forbid(unsafe_code)]
pub mod config;
mod index;
pub use index::index;

mod api;
mod auth;
pub use api::serve;
