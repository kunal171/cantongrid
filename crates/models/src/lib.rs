//! Deterministic typed model definitions. Compilation never authenticates a source or caller.
#![forbid(unsafe_code)]
mod definition;
mod error;
pub use definition::{Catalog, Expression, FieldType, Filter, Model};
pub use error::ModelError;
