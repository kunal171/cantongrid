//! Source-neutral, fixture-tested primitives. No live connector, authentication provider or server.
#![forbid(unsafe_code)]

pub mod authorization;
pub mod config;
pub mod document;
mod envelope;
pub mod exercise;
pub mod fixture;
pub mod lifecycle;
pub mod lineage;
pub mod value;
pub mod visibility;
