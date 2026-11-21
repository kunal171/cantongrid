//! Shared REST/GraphQL database reads; caller identity is never accepted from query filters.
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;

/// Queryable canonical records. This is not an arbitrary SQL/expression interface.
#[derive(Debug, Clone, Copy)]
pub enum PageKind {
    Active,
    History,
}

/// Backend page keys are internal identities; HTTP must wrap them in scoped, authenticated cursors.
#[derive(serde::Serialize)]
pub struct ReadPage {
    pub items: Vec<serde_json::Value>,
    pub visible_total: i64,
    pub next_after: Option<String>,
    pub checkpoint: Option<String>,
    pub observed_age_seconds: Option<i64>,
    pub first_checkpoint: Option<String>,
}

impl std::fmt::Debug for ReadPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReadPage([redacted])")
    }
}

impl Store {
    /// Apply scope and visibility before count and keyset paging, in one stable database snapshot.
    /// Grants/leases must come from trusted providers; fixture party strings never authenticate.
    pub fn read_page(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        kind: PageKind,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ReadPage, StoreError> {
        let scope = plan.context().binding();
        let parties: Vec<String> = lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        if !(1..=100).contains(&limit)
            || after.is_some_and(|p| p.is_empty() || p.chars().any(char::is_control))
        {
            return Err(StoreError::Limit);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let key = stream_key(plan).0;
        let definition = tx.query_opt(
            "SELECT definition,checkpoint,GREATEST(0,extract(epoch FROM now()-observed_at)::bigint),(SELECT position FROM cantongrid.batches WHERE stream_key=$1 ORDER BY revision LIMIT 1) FROM cantongrid.streams WHERE stream_key=$1",
            &[&key],
        )?;
        let checkpoint = definition
            .as_ref()
            .and_then(|r| r.get::<_, Option<String>>(1));
        let mut first_checkpoint = definition
            .as_ref()
            .and_then(|r| r.get::<_, Option<String>>(3));
        if let Some(row) = tx.query_opt("SELECT manifest->>'checkpoint' FROM cantongrid.selection_upgrades WHERE stream_key=$1 ORDER BY ((manifest->>'checkpoint')::jsonb->>'boundary')::bigint DESC LIMIT 1", &[&key])? {
            first_checkpoint = Some(row.get(0));
        }
        let observed_age_seconds = definition.as_ref().map(|r| r.get::<_, i64>(2));
        if let Some(row) = definition
            && row.get::<_, serde_json::Value>(0) != plan.definition()
        {
            return Err(StoreError::PlanChanged);
        }
        let sql = match kind {
            PageKind::Active => include_str!("query_active.sql"),
            PageKind::History => include_str!("query_history.sql"),
        };
        // Fetch one extra authorized row; private rows cannot influence has-more/page totals.
        let fetch = limit as i64 + 1;
        let row = tx.query_one(sql, &[&key, &parties, &after, &fetch])?;
        let visible_total: i64 = row.get(0);
        let encoded: serde_json::Value = row.get(1);
        let mut rows = encoded
            .as_array()
            .ok_or(StoreError::CorruptJournal)?
            .clone();
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next_after = if more {
            Some(
                rows.last()
                    .and_then(|r| r.get("key"))
                    .and_then(|k| k.as_str())
                    .ok_or(StoreError::CorruptJournal)?
                    .to_owned(),
            )
        } else {
            None
        };
        let items = rows
            .into_iter()
            .map(|mut row| {
                row.get_mut("value")
                    .map(serde_json::Value::take)
                    .ok_or(StoreError::CorruptJournal)
            })
            .collect::<Result<_, _>>()?;
        tx.commit()?;
        // Permission at query start cannot authorize delivery after the lease expires.
        if lease.read_parties(&scope, caller, Instant::now()).is_none() {
            return Err(StoreError::Authorization);
        }
        Ok(ReadPage {
            items,
            visible_total,
            next_after,
            checkpoint,
            observed_age_seconds,
            first_checkpoint,
        })
    }
}
