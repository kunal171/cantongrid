//! Caller-scoped one-hop lineage reads; no source witness or internal proof lists are exposed.
use crate::{ReadPage, Store, StoreError, journal::stream_key};
use cantongrid_core::config::Plan;

/// Bounded one-hop traversal; no approximation of transitive ancestry through hidden nodes.
#[derive(Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineageDirection {
    Parents,
    Children,
}

/// Validated query data is separate from source/caller authority. Cursor signing belongs to transport.
pub struct LineageQuery<'a> {
    pub root: &'a str,
    pub direction: LineageDirection,
    pub after: Option<&'a str>,
    pub limit: usize,
}
impl LineageDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parents => "parents",
            Self::Children => "children",
        }
    }
}

impl Store {
    /// Check the visible root, independent endpoints and all source-proof dependencies before paging.
    pub fn read_lineage(
        &mut self,
        plan: &Plan,
        caller: &cantongrid_core::visibility::CallerGrants,
        lease: &cantongrid_core::authorization::SourceLease,
        query: LineageQuery<'_>,
    ) -> Result<ReadPage, StoreError> {
        let LineageQuery {
            root,
            direction,
            after,
            limit,
        } = query;
        let scope = plan.context().binding();
        let parties = lease
            .read_parties(&scope, caller, std::time::Instant::now())
            .ok_or(StoreError::Authorization)?;
        let parties: Vec<String> = parties.iter().map(|p| p.as_str().to_owned()).collect();
        if !plan.includes_lineage() {
            return Err(StoreError::LineageUnavailable);
        }
        if root.is_empty()
            || root.len() > 1024
            || root.trim() != root
            || root.chars().any(char::is_control)
            || !(1..=100).contains(&limit)
            || after.is_some_and(|s| {
                s.len() != 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return Err(StoreError::Limit);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(cantongrid_postgres::postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let prefix = crate::live::prefix(&mut tx, plan)?;
        let key = stream_key(plan).0;
        if tx.query_opt("SELECT 1 FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2 AND witnesses && $3::text[]", &[&key,&root,&parties])?.is_none(){return Err(StoreError::LineageUnavailable);}
        let row = tx.query_one(
            include_str!("read.sql"),
            &[
                &key,
                &parties,
                &root,
                &matches!(direction, LineageDirection::Children),
                &after,
                &(limit as i64 + 1),
            ],
        )?;
        let total: i64 = row.get(0);
        let encoded: serde_json::Value = row.get(1);
        let mut records = encoded
            .as_array()
            .ok_or(StoreError::CorruptJournal)?
            .clone();
        let more = records.len() > limit;
        records.truncate(limit);
        let next = if more {
            Some(
                records
                    .last()
                    .and_then(|r| r["key"].as_str())
                    .ok_or(StoreError::CorruptJournal)?
                    .to_owned(),
            )
        } else {
            None
        };
        let items = records
            .into_iter()
            .map(|mut r| {
                r.get_mut("value")
                    .map(serde_json::Value::take)
                    .ok_or(StoreError::CorruptJournal)
            })
            .collect::<Result<_, _>>()?;
        let observed=tx.query_one("SELECT GREATEST(0,extract(epoch FROM now()-observed_at)::bigint) FROM cantongrid.streams WHERE stream_key=$1", &[&key])?.get(0);
        tx.commit()?;
        if lease
            .read_parties(&scope, caller, std::time::Instant::now())
            .is_none()
        {
            return Err(StoreError::Authorization);
        }
        Ok(ReadPage {
            items,
            visible_total: total,
            next_after: next,
            checkpoint: prefix.and_then(|(_, c)| c),
            observed_age_seconds: Some(observed),
            first_checkpoint: None,
        })
    }
}
