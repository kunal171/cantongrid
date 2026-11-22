//! Read-only committed metadata and sanitized diagnostics; no raw source envelope is public.
mod progress;
mod projection;
mod transaction;
use crate::{ReadPage, StoreError};

/// Validate identities without admitting empty/ambiguous path values or unbounded cursors.
pub(super) fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
/// Inspector pages deliberately carry no global checkpoints or observation timestamps.
pub(super) fn page(items: Vec<serde_json::Value>, total: i64, next: Option<String>) -> ReadPage {
    ReadPage {
        items,
        visible_total: total,
        next_after: next,
        checkpoint: None,
        observed_age_seconds: None,
        first_checkpoint: None,
    }
}
/// Restrict chronological cursor ordinals to this bounded transaction's *visible* record space.
pub(super) fn ordinal(after: Option<&str>) -> Result<i64, StoreError> {
    after
        .map(|s| {
            s.parse::<i64>()
                .ok()
                .filter(|n| *n >= 1 && *n <= 128 && n.to_string() == s)
                .ok_or(StoreError::ModelQuery)
        })
        .transpose()
        .map(|n| n.unwrap_or(0))
}

#[cfg(test)]
mod tests;
