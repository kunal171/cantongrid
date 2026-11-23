//! Only complete scoped runs can construct reports; raw rows and global positions are never serialized.
use cantongrid_models::reconciliation::{FixtureResult, Policy, PolicyIdentity};
use serde::Serialize;

/// Complete per-check counts over one bounded set of selected per-contract projections.
#[derive(Debug, Serialize)]
struct CheckSummary {
    name: String,
    satisfied: String,
    violated: String,
    unavailable: String,
}
/// A definition identity, never a public cursor or claim about the current ledger head.
#[derive(Debug, Serialize)]
struct ModelIdentity {
    namespace: String,
    name: String,
    revision: String,
    fingerprint: String,
}
/// Storage-constructed, serialization-only evidence. Coverage and arithmetic outcome are independent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReport {
    format_version: u32,
    policy: PolicyIdentity,
    model: ModelIdentity,
    definition_fingerprint: String,
    status: &'static str,
    comparison: FixtureResult,
    coverage: &'static str,
    complete_within_configured_scope: bool,
    contracts: String,
    checks: Vec<CheckSummary>,
}
/// Bounded counters remain private until the complete snapshot and authorization checks succeed.
#[derive(Default)]
pub(super) struct Counter {
    pub satisfied: u32,
    pub violated: u32,
    pub unavailable: u32,
}
impl Counter {
    /// At most 128 inputs reach this accumulator; exact counters cannot overflow their u32 range.
    pub fn add(&mut self, result: FixtureResult) {
        match result {
            FixtureResult::Satisfied => self.satisfied += 1,
            FixtureResult::Violated => self.violated += 1,
            FixtureResult::Unavailable => self.unavailable += 1,
        }
    }
}
impl ReconciliationReport {
    /// Construct only after complete input/projection verification; unavailable arithmetic dominates violations.
    pub(super) fn complete(
        binding: &super::BoundModel,
        policy: &Policy,
        fingerprint: String,
        contracts: u32,
        counters: Vec<Counter>,
    ) -> Self {
        let comparison = if counters.iter().any(|c| c.unavailable > 0) {
            FixtureResult::Unavailable
        } else if counters.iter().any(|c| c.violated > 0) {
            FixtureResult::Violated
        } else {
            FixtureResult::Satisfied
        };
        Self {
            format_version: 1,
            policy: PolicyIdentity {
                name: policy.name().into(),
                revision: policy.revision().to_string(),
                fingerprint: policy.fingerprint().into(),
            },
            model: ModelIdentity {
                namespace: binding.namespace.clone(),
                name: binding.model.name.clone(),
                revision: binding.model.revision.to_string(),
                fingerprint: binding.fingerprint.clone(),
            },
            definition_fingerprint: fingerprint,
            status: "evaluated",
            comparison,
            coverage: "retained_indexed_prefix",
            complete_within_configured_scope: true,
            contracts: contracts.to_string(),
            checks: policy
                .check_names()
                .zip(counters)
                .map(|(name, c)| CheckSummary {
                    name: name.into(),
                    satisfied: c.satisfied.to_string(),
                    violated: c.violated.to_string(),
                    unavailable: c.unavailable.to_string(),
                })
                .collect(),
        }
    }
}
