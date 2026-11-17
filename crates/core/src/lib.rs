//! Source-neutral core primitives. No connector, authentication provider or server exists yet.
#![forbid(unsafe_code)]

pub mod visibility;

pub mod authorization;
pub mod config;

pub mod value;

pub mod lifecycle;
