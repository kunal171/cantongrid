//! Finite fixture diagnostics never upgrade supplied data into trusted source evidence.
use super::Policy;
use crate::ModelError;
use serde::Serialize;

/// A mathematical comparison of supplied fixture inputs, not a finding about an authenticated ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureResult {
    Satisfied,
    Violated,
    Unavailable,
}

/// Only configured identities and finite error codes leave the evaluator; values/paths stay private.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckFinding {
    pub name: String,
    pub fixture_comparison: FixtureResult,
    pub error_code: Option<&'static str>,
}
/// An immutable policy identity can be recorded without serializing its literals or expression paths.
#[derive(Debug, Serialize)]
pub struct PolicyIdentity {
    pub name: String,
    pub revision: String,
    pub fingerprint: String,
}
/// Serialization only: untrusted JSON cannot deserialize an allegedly verified reconciliation receipt.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    format_version: u32,
    policy: PolicyIdentity,
    status: &'static str,
    coverage: &'static str,
    source_scope: Option<String>,
    source_offset: Option<String>,
    complete_within_configured_scope: bool,
    checks: Vec<CheckFinding>,
}
impl Report {
    /// Every offline outcome remains unverified, even when every mathematical comparison is satisfied.
    pub(super) fn fixture(policy: &Policy, checks: Vec<CheckFinding>) -> Self {
        Self {
            format_version: 1,
            policy: PolicyIdentity {
                name: policy.name().into(),
                revision: policy.revision().to_string(),
                fingerprint: policy.fingerprint().into(),
            },
            status: "unverified",
            coverage: "supplied_fixture_only",
            source_scope: None,
            source_offset: None,
            complete_within_configured_scope: false,
            checks,
        }
    }
    /// Consumers can inspect fixture outcomes but cannot use them as authenticated source capabilities.
    pub fn checks(&self) -> &[CheckFinding] {
        &self.checks
    }
}
/// Error classifications are stable and redacted; driver/parser text and supplied values are excluded.
pub(super) fn code(error: ModelError) -> &'static str {
    match error {
        ModelError::Document => "document",
        ModelError::Definition => "definition",
        ModelError::Type => "type",
        ModelError::Path => "path",
        ModelError::Missing => "missing",
        ModelError::Overflow => "overflow",
        ModelError::Limit => "limit",
        ModelError::Query => "query",
    }
}
