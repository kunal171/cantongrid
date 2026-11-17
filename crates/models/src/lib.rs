//! Deterministic typed model definitions, checked exact evaluation and managed schema planning.
//! Compiling/evaluating a definition never authenticates a source or API caller.
#![forbid(unsafe_code)]
mod definition;
mod error;
pub use definition::{Catalog, Expression, FieldType, Filter, Model};
pub use error::ModelError;

mod arithmetic;
mod evaluate;
mod sql;
pub use sql::SqlPlan;

#[cfg(test)]
mod tests;
