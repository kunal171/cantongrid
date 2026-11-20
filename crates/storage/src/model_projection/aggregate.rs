//! Whole-group authorization over transactionally maintained current-state contributions.
use super::{BoundModel, ModelPage, installed};
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::IsolationLevel;
use std::time::Instant;

impl Store {
    /// Count/sum all contributors, then expose only groups whose every witness clause is met.
    /// Input-level caller filtering would silently turn complete totals into partial totals.
    #[allow(clippy::too_many_arguments)]
    pub fn read_aggregate(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ModelPage, StoreError> {
        if !super::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        let scope = plan.context().binding();
        let parties: Vec<_> = lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        if !(1..=25).contains(&limit) {
            return Err(StoreError::Limit);
        }
        if let Some(key) = after {
            binding
                .model
                .validate_aggregate_key(key)
                .map_err(|_| StoreError::ModelQuery)?;
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ModelBinding)?;
        let key = stream_key(plan).0;
        let active = tx.query_opt("SELECT revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3",
            &[&key,&binding.namespace,&binding.model.name])?.ok_or(StoreError::ModelBinding)?;
        if active.get::<_, i64>(0) != i64::from(binding.model.revision)
            || active.get::<_, String>(1) != binding.fingerprint
        {
            return Err(StoreError::ModelBinding);
        }
        let model = installed::read(
            &mut tx,
            &binding.namespace,
            &binding.model.name,
            binding.model.revision,
        )?;
        if model.sql.fingerprint != binding.fingerprint || !plan.selects(&model.model.template) {
            return Err(StoreError::ModelBinding);
        }
        let compiled = model
            .model
            .aggregate_plan()
            .map_err(|_| StoreError::ModelQuery)?;
        let template =
            serde_json::to_value(&model.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        // An overlap satisfies one contract's OR clause. bool_and enforces every
        // contributor's clause; missing/empty witnesses fail rather than disappearing.
        let grouped = format!(
            "WITH grouped AS MATERIALIZED (SELECT {},{} AS _cg_key,{},
            bool_and(coalesce(c.witnesses && $2::text[],false)) AS _cg_allowed
            FROM {} m JOIN cantongrid.contracts c ON c.stream_key=m.\"_cg_stream_key\"
                AND c.contract_id=m.\"_cg_contract_id\"
            WHERE c.stream_key=$1 AND c.active AND c.template=$3
            GROUP BY {})",
            compiled.columns,
            compiled.key,
            compiled.measures,
            model.relation(),
            compiled.groups
        );
        let total: i64 = tx
            .query_one(
                &format!("{grouped} SELECT count(*) FROM grouped g WHERE g._cg_allowed"),
                &[&key, &parties, &template],
            )?
            .get(0);
        let fetch = limit as i64 + 1;
        // CASE guards bounded numeric casts from optimizer movement: an unauthorized
        // overflowing group must neither reveal a total nor break a caller's visible query.
        let sql = format!(
            "{grouped} SELECT g._cg_key::text,
            CASE WHEN g._cg_allowed THEN jsonb_build_object('groupKey',g._cg_key::text,
                'values',jsonb_build_object({})) ELSE NULL END
            FROM grouped g WHERE g._cg_allowed AND ($4::text IS NULL OR
                g._cg_key::text COLLATE \"C\" > $4::text COLLATE \"C\")
            ORDER BY g._cg_key::text COLLATE \"C\" LIMIT $5",
            compiled.values
        );
        let rows = tx.query(&sql, &[&key, &parties, &template, &after, &fetch])?;
        let more = rows.len() > limit;
        let mut items = Vec::new();
        let mut last = None;
        let mut bytes = 0;
        for row in rows.into_iter().take(limit) {
            let group: String = row.get(0);
            model
                .model
                .validate_aggregate_key(&group)
                .map_err(|_| StoreError::Limit)?;
            let value: serde_json::Value = row.get(1);
            bytes += serde_json::to_vec(&value)
                .map_err(|_| StoreError::ModelCorrupt)?
                .len();
            if bytes > 2 * 1024 * 1024 {
                return Err(StoreError::Limit);
            }
            if more && items.len() + 1 == limit {
                last = Some(group);
            }
            items.push(value);
        }
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(ModelPage {
            items,
            visible_total: total.to_string(),
            next_after: if more { last } else { None },
            fingerprint: binding.fingerprint.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
