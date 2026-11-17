//! Machine-readable source uncertainty; observed SQL rows never imply current authorization.
use serde::Serialize;

/// Complete history is deliberately not constructible from this metadata-only probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Unknown,
    PartialHistory,
}

/// Stable reason codes can be interpreted without parsing a human error message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageReason {
    NoSnapshot,
    PruningMarkerPresent,
    ArchiveWitnessEvidenceUnavailable,
    CurrentSourceAuthorizationUnverified,
    ConcurrentHandoffUnverified,
    SourceFreshnessUnverified,
}

/// Operator metadata about the store view, not an authenticated API principal's permitted scope.
#[derive(Debug, Serialize)]
pub struct Coverage {
    pub status: CoverageStatus,
    pub reasons: Vec<CoverageReason>,
    pub snapshot_available: bool,
    pub scope_basis: &'static str,
    pub current_source_authorization_verified: bool,
    pub source_freshness_verified: bool,
}

impl Coverage {
    /// Preserve all known limitations even if a source returns a nonempty, internally consistent view.
    pub(crate) fn from_sql_observation(latest: Option<i64>, pruned: Option<i64>) -> Self {
        let mut reasons = vec![
            CoverageReason::ArchiveWitnessEvidenceUnavailable,
            CoverageReason::CurrentSourceAuthorizationUnverified,
            CoverageReason::ConcurrentHandoffUnverified,
            CoverageReason::SourceFreshnessUnverified,
        ];
        if latest.is_none() {
            reasons.push(CoverageReason::NoSnapshot);
        }
        if pruned.is_some() {
            reasons.push(CoverageReason::PruningMarkerPresent);
        }
        Self {
            status: if pruned.is_some() {
                CoverageStatus::PartialHistory
            } else {
                CoverageStatus::Unknown
            },
            reasons,
            snapshot_available: latest.is_some(),
            scope_basis: "sql_store_view_only",
            current_source_authorization_verified: false,
            source_freshness_verified: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_available_snapshot_never_implies_authorization_freshness_or_complete_history() {
        for latest in [None, Some(0), Some(943)] {
            let coverage = Coverage::from_sql_observation(latest, None);
            assert_eq!(coverage.status, CoverageStatus::Unknown);
            assert!(!coverage.current_source_authorization_verified);
            assert!(!coverage.source_freshness_verified);
            assert!(
                coverage
                    .reasons
                    .contains(&CoverageReason::ArchiveWitnessEvidenceUnavailable)
            );
            assert_eq!(
                coverage.reasons.contains(&CoverageReason::NoSnapshot),
                latest.is_none()
            );
        }
    }
    #[test]
    fn pruning_is_disclosed_even_when_the_snapshot_is_available() {
        let coverage = Coverage::from_sql_observation(Some(943), Some(100));
        assert_eq!(coverage.status, CoverageStatus::PartialHistory);
        assert!(
            coverage
                .reasons
                .contains(&CoverageReason::PruningMarkerPresent)
        );
        assert!(!coverage.current_source_authorization_verified);
    }
}
