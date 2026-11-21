//! Offline deterministic schema synchronization. Developer expressions remain explicit;
//! no Ledger connection, database migration or executable LF interpretation occurs here.
#![forbid(unsafe_code)]
mod bundle;
mod compare;
pub use bundle::Bundle;
mod plan;
mod resolve;
pub use compare::Change;
pub use plan::{Diagnostic, Report, SyncPlan, plan};

use serde::Serialize;

/// Safe operator diagnostics omit source literals, parser internals and input file paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncError {
    Identity,
    MissingField,
    UnsupportedType,
    Incompatible,
    Limit,
    Definition,
}
impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Identity => "missing or ambiguous package/template identity",
            Self::MissingField => "selected field is absent from the contract signature",
            Self::UnsupportedType => "selected LF type cannot be represented by this projection",
            Self::Incompatible => {
                "selected contract type or semantics changed; explicit model review required"
            }
            Self::Limit => "schema synchronization resource limit exceeded",
            Self::Definition => "invalid model synchronization definition",
        })
    }
}
impl std::error::Error for SyncError {}
