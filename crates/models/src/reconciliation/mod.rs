//! Offline arithmetic policies do not authenticate inputs, prove completeness or certify ledger state.
mod definition;
mod evaluate;
mod preflight;
mod report;
pub use definition::Policy;
pub use preflight::ModelPreflight;
pub use report::{CheckFinding, FixtureResult, PolicyIdentity, Report};

#[cfg(test)]
mod definition_tests;

#[cfg(test)]
mod evaluate_tests;

#[cfg(test)]
mod preflight_tests;
