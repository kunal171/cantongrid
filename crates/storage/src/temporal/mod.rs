//! Bounded canonical state at an acknowledged source offset; current grants always apply.
pub(crate) mod coverage;
use crate::{ReadPage, Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;

/// Requested offsets are source-local integer boundaries, not timestamps or application dates.
pub struct TemporalQuery<'a> {
    pub offset: i64,
    pub after: Option<&'a str>,
    pub limit: usize,
}
impl Store {
    /// Reconstruct membership from retained immutable creations and their observed archive offsets.
    /// Missing coverage fails explicitly; a confident zero is returned only for a proven boundary.
    pub fn read_as_of(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        query: TemporalQuery<'_>,
    ) -> Result<ReadPage, StoreError> {
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        if !plan.is_live() || plan.source_binding().is_none() {
            return Err(StoreError::TemporalUnavailable);
        }
        let scope = plan.context().binding();
        let parties: Vec<_> = lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let (revision, checkpoint) =
            crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::TemporalUnavailable)?;
        let checkpoint = checkpoint.ok_or(StoreError::TemporalUnavailable)?;
        let key = stream_key(plan).0;
        let page = read_at(&mut tx, &key, revision, &checkpoint, &parties, query)?;
        tx.commit()?;
        if !crate::model_projection::authorized(plan, lease)
            || lease.read_parties(&scope, caller, Instant::now()).is_none()
        {
            return Err(StoreError::Authorization);
        }
        Ok(page)
    }
}

/// Reuse canonical membership/coverage in an existing stable transaction, including bootstrap's writer lock.
pub(crate) fn read_at(
    tx: &mut cantongrid_postgres::postgres::Transaction<'_>,
    key: &str,
    revision: i64,
    checkpoint: &str,
    parties: &[String],
    query: TemporalQuery<'_>,
) -> Result<ReadPage, StoreError> {
    if query.offset < 0
        || !(1..=25).contains(&query.limit)
        || query
            .after
            .is_some_and(|s| s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
    {
        return Err(StoreError::ModelQuery);
    }
    let coverage = coverage::observed(tx, key, revision, checkpoint, query.offset)?;
    if query.offset > coverage.through {
        return Err(StoreError::TemporalUnavailable);
    }
    let fetch = query.limit as i64 + 1;
    let row = tx.query_one(
        include_str!("state.sql"),
        &[&key, &parties, &query.offset, &query.after, &fetch],
    )?;
    let total = row.get(0);
    let encoded: serde_json::Value = row.get(1);
    let mut entries = encoded
        .as_array()
        .ok_or(StoreError::CorruptJournal)?
        .clone();
    let more = entries.len() > query.limit;
    entries.truncate(query.limit);
    let next = if more {
        Some(
            entries
                .last()
                .and_then(|v| v["key"].as_str())
                .ok_or(StoreError::CorruptJournal)?
                .to_owned(),
        )
    } else {
        None
    };
    let items = entries
        .into_iter()
        .map(|mut v| {
            v.get_mut("value")
                .map(serde_json::Value::take)
                .ok_or(StoreError::CorruptJournal)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ReadPage {
        items,
        visible_total: total,
        next_after: next,
        checkpoint: Some(checkpoint.to_owned()),
        observed_age_seconds: None,
        first_checkpoint: Some(coverage.floor_position),
    })
}

#[cfg(test)]
mod tests;
