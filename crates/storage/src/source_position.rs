//! Exact schema041 positions shared by temporal coverage and complete-boundary consumers.
use crate::StoreError;
use serde::Deserialize;

/// A source-local cursor; its private PK breaks ties and must never become a public feed ordinal.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PqsPosition {
    /// Initial active-contract snapshot pages cannot establish a completed live handoff.
    pub snapshot: bool,
    /// Last fully acknowledged source offset.
    pub boundary: i64,
    /// Upper bound of the interval currently being paged.
    pub through: i64,
    /// Offset of the last row processed inside that interval.
    pub offset: i64,
    /// Private tie-breaker; i64::MAX marks completion of all rows at the boundary.
    pub pk: i64,
}
impl PqsPosition {
    /// Decode only the pinned cursor shape; unknown/duplicate fields and oversized text fail closed.
    pub fn decode(text: &str) -> Result<Self, StoreError> {
        if text.len() > 4096 {
            return Err(StoreError::TemporalUnavailable);
        }
        let p: Self = serde_json::from_str(text).map_err(|_| StoreError::TemporalUnavailable)?;
        if p.boundary < 0
            || p.through < p.boundary
            || p.offset < p.boundary
            || p.offset > p.through
            || p.pk < 0
            || (p.snapshot && p.boundary != p.through)
            || (!p.snapshot && p.through == p.boundary && p.pk != i64::MAX)
        {
            return Err(StoreError::TemporalUnavailable);
        }
        Ok(p)
    }

    /// Require every effect through B to be acknowledged before anchoring a new baseline/feed pair.
    /// For example, boundary=10, through=20, offset=15 is valid paging but not a complete handoff.
    pub fn complete_boundary(&self) -> Option<i64> {
        (!self.snapshot
            && self.boundary == self.through
            && self.offset == self.boundary
            && self.pk == i64::MAX)
            .then_some(self.boundary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keep exact integer boundaries, including values above JavaScript's safe integer range.
    #[test]
    fn complete_boundaries_preserve_zero_and_exact_i64_values() {
        for boundary in [0, 9_007_199_254_740_993, i64::MAX] {
            let text = serde_json::json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string();
            assert_eq!(
                PqsPosition::decode(&text).unwrap().complete_boundary(),
                Some(boundary)
            );
        }
    }

    /// A readable temporal prefix during forward paging is insufficient to start an atomic handoff.
    #[test]
    fn snapshot_and_partial_intervals_cannot_become_complete_boundaries() {
        for text in [
            r#"{"snapshot":true,"boundary":10,"through":10,"offset":10,"pk":1}"#,
            r#"{"snapshot":false,"boundary":10,"through":20,"offset":15,"pk":1}"#,
            r#"{"snapshot":false,"boundary":10,"through":20,"offset":20,"pk":9223372036854775807}"#,
        ] {
            assert_eq!(PqsPosition::decode(text).unwrap().complete_boundary(), None);
        }
    }

    /// Future fields, aliases and ambiguous duplicate keys require explicit profile review.
    #[test]
    fn unknown_duplicate_and_inexact_cursor_fields_are_rejected() {
        let valid =
            r#"{"snapshot":false,"boundary":10,"through":10,"offset":10,"pk":9223372036854775807}"#;
        for text in [
            valid.replace('}', ",\"future\":true}"),
            valid.replace('}', ",\"boundary\":10}"),
            valid.replace("\"offset\":10", "\"offset\":10.0"),
            valid.replace("\"boundary\":10", "\"boundary\":\"10\""),
            valid.replace("9223372036854775807", "9223372036854775808"),
            " ".repeat(4097),
        ] {
            assert!(PqsPosition::decode(&text).is_err());
        }
    }

    /// Invalid ranges or an unfinished same-offset tie cannot prove complete coverage.
    #[test]
    fn invalid_ranges_and_incomplete_terminal_ties_are_rejected() {
        for (boundary, through, offset, pk) in [
            (10, 9, 10, 1),
            (10, 20, 9, 1),
            (10, 20, 21, 1),
            (-1, 0, 0, 1),
            (10, 10, 10, 1),
            (10, 20, 15, -1),
        ] {
            let text = serde_json::json!({"snapshot":false,"boundary":boundary,"through":through,"offset":offset,"pk":pk}).to_string();
            assert!(PqsPosition::decode(&text).is_err());
        }
    }
}
