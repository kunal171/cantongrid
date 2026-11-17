//! Source-neutral, fixture-tested primitives. No live connector, authentication provider or server.
#![forbid(unsafe_code)]

pub mod authorization;
pub mod config;
mod document;
pub mod fixture;
pub mod lifecycle;
pub mod value;
pub mod visibility;
