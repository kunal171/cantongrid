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
pub use sql::{SqlColumn, SqlPlan};

#[cfg(test)]
mod tests;

mod aggregation;
mod client;
pub use aggregation::{AggregatePlan, Aggregation, Measure};

mod query;
pub use query::{
    Comparison, Direction, QueryField, QueryOptions, QueryValue, Sort, Sorts, ValidatedQuery,
};

mod relationship;
pub use relationship::{Relationship, RelationshipTarget};

mod join;
pub use join::{JoinFields, JoinKind, JoinProjection};

#[cfg(test)]
mod join_tests;
