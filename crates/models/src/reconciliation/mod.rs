//! Offline arithmetic policies do not authenticate inputs, prove completeness or certify ledger state.
mod definition;
mod evaluate;
mod report;
pub use definition::Policy;
pub use report::{CheckFinding, FixtureResult, PolicyIdentity, Report};

#[cfg(test)]
mod definition_tests;

#[cfg(test)]
mod evaluate_tests;
