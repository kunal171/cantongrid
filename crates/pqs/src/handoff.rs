//! Retained-window reconciliation for the installed SQL profile, without payload copying.
use crate::{InspectError, validate_source};
use cantongrid_postgres::{connect_env, postgres::IsolationLevel};

/// PQS-specific participant-local integer bounds. They never become a generic global height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedWindow {
    after: i64,
    through: i64,
}
impl RetainedWindow {
    /// Require an available, unpruned interval; a gap is an error, never a silent skip.
    pub fn new(
        oldest: i64,
        latest: i64,
        pruned: Option<i64>,
        after: i64,
        through: i64,
    ) -> Result<Self, InspectError> {
        if oldest < 0
            || latest < oldest
            || through < after
            || through > latest
            || after < 0
            || pruned.is_some_and(|p| p < 0 || p > latest)
        {
            return Err(InspectError::Bounds);
        }
        // Treat the pruning boundary itself conservatively until boundary retention is proven.
        if after < oldest || pruned.is_some_and(|p| after <= p) {
            return Err(InspectError::Gap);
        }
        Ok(Self { after, through })
    }
}

/// A set-consistency observation, not proof of concurrent publication safety or current grants.
#[derive(Debug, serde::Serialize)]
pub struct Reconciliation {
    pub after_offset: i64,
    pub through_offset: i64,
    pub baseline_rows: i64,
    pub created_rows: i64,
    pub archived_rows: i64,
    pub observed_active_rows: i64,
    pub missing_rows: i64,
    pub unexpected_rows: i64,
    pub retained_interval_consistent: bool,
    pub concurrent_handoff_verified: bool,
    pub coverage: &'static str,
}

/// Verify active(B1) = (active(B0) union creates(B0,B1]) minus archives(B0,B1].
/// All reads share one SQL snapshot; only aggregate counts leave the restricted source connection.
pub fn reconcile_env(
    variable: &str,
    after: i64,
    through: Option<i64>,
) -> Result<Reconciliation, InspectError> {
    let mut client = connect_env(variable)?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    validate_source(&mut tx)?;
    let bounds = tx.query_one(
        "SELECT public.oldest_offset(),public.latest_offset(),public.pruned_offset()",
        &[],
    )?;
    let oldest: Option<i64> = bounds.get(0);
    let latest: Option<i64> = bounds.get(1);
    let pruned: Option<i64> = bounds.get(2);
    let oldest = oldest.ok_or(InspectError::Bounds)?;
    let latest = latest.ok_or(InspectError::Bounds)?;
    let window = RetainedWindow::new(oldest, latest, pruned, after, through.unwrap_or(latest))?;
    let row = tx.query_one(
        include_str!("handoff.sql"),
        &[&window.after, &window.through],
    )?;
    let missing_rows: i64 = row.get(4);
    let unexpected_rows: i64 = row.get(5);
    let report = Reconciliation {
        after_offset: window.after,
        through_offset: window.through,
        baseline_rows: row.get(0),
        created_rows: row.get(1),
        archived_rows: row.get(2),
        observed_active_rows: row.get(3),
        missing_rows,
        unexpected_rows,
        retained_interval_consistent: missing_rows == 0 && unexpected_rows == 0,
        concurrent_handoff_verified: false,
        coverage: "retained_interval_only",
    };
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_pruned_regressed_and_future_boundaries() {
        for (oldest, latest, pruned, after, through) in [
            (-1, 10, None, 0, 10),
            (0, -1, None, 0, 0),
            (5, 10, None, 4, 10),
            (0, 10, None, 7, 6),
            (0, 10, None, 0, 11),
            (0, 10, Some(5), 4, 10),
            (0, 10, Some(11), 5, 10),
            (0, 10, Some(-1), 5, 10),
        ] {
            assert!(RetainedWindow::new(oldest, latest, pruned, after, through).is_err());
        }
        assert!(RetainedWindow::new(0, 10, Some(5), 6, 10).is_ok());
        assert!(RetainedWindow::new(0, 10, None, 10, 10).is_ok());
        assert_eq!(
            RetainedWindow::new(5, 10, None, 4, 10),
            Err(InspectError::Gap)
        );
        assert_eq!(
            RetainedWindow::new(0, 10, Some(5), 5, 10),
            Err(InspectError::Gap)
        );
    }
    #[test]
    #[ignore = "requires existing schema-041 source and CANTONGRID_PQS_URL"]
    fn retained_historical_window_reconciles_without_inventing_concurrency_evidence() {
        let report = reconcile_env("CANTONGRID_PQS_URL", 80, None).unwrap();
        assert!(report.retained_interval_consistent);
        assert!(!report.concurrent_handoff_verified);
        let empty = reconcile_env(
            "CANTONGRID_PQS_URL",
            report.through_offset,
            Some(report.through_offset),
        )
        .unwrap();
        assert_eq!(empty.created_rows, 0);
        assert_eq!(empty.archived_rows, 0);
        assert!(empty.retained_interval_consistent);
    }
}
