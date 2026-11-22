//! Transactions are flat partial views: ordering cannot expose gaps left by hidden source nodes.
use crate::{ReadPage, Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;

impl Store {
    /// Reconcile only recorded metadata with independently visible indexed endpoints.
    /// Unindexed transients and rejected submissions have no guessed public representation.
    pub fn inspect_transaction(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        transaction: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ReadPage, StoreError> {
        if !super::identity(transaction) || !(1..=25).contains(&limit) {
            return Err(StoreError::ModelQuery);
        }
        let after = super::ordinal(after)?;
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        if !plan.includes_lineage() {
            return Err(StoreError::InspectorUnavailable);
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
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::InspectorUnavailable)?;
        let row = tx.query_one(
            include_str!("transaction.sql"),
            &[
                &stream_key(plan).0,
                &parties,
                &transaction,
                &after,
                &(limit as i64 + 1),
            ],
        )?;
        let total: i64 = row.get(0);
        if total == 0 {
            return Err(StoreError::InspectorUnavailable);
        }
        if total > 128 {
            return Err(StoreError::Limit);
        }
        let value: serde_json::Value = row.get(1);
        let mut records = value.as_array().ok_or(StoreError::CorruptJournal)?.clone();
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
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(super::page(items, total, next))
    }
}
