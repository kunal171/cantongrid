//! Offline arithmetic policies do not authenticate inputs, prove completeness or certify ledger state.
mod definition;
pub use definition::Policy;

#[cfg(test)]
mod definition_tests;
