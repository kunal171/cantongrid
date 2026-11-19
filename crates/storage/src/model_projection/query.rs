//! Read projected current state through canonical contract visibility, before counts and paging.
use super::installed;
use super::query_plan::QueryPlan;
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_models::{Model, QueryOptions};
use cantongrid_postgres::postgres::IsolationLevel;
use cantongrid_postgres::postgres::types::ToSql;
use serde::Serialize;
use std::time::Instant;

/// Trusted backend model identity. Definitions/literals are never serialized into public responses.
#[derive(Clone)]
pub struct BoundModel {
    pub namespace: String,
    pub model: Model,
    pub fingerprint: String,
}

/// Exact model values; counts and keys are computed after caller visibility.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPage {
    pub items: Vec<serde_json::Value>,
    pub visible_total: String,
    pub next_after: Option<String>,
    pub fingerprint: String,
}

impl Store {
    /// Discover only this physical plan's bindings after source/caller validation. This returns
    /// backend definitions for schema construction, not contract rows or public registry access.
    pub fn bound_models(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
    ) -> Result<Vec<BoundModel>, StoreError> {
        let scope = plan.context().binding();
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ModelBinding)?;
        let rows = tx.query("SELECT namespace,name,revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 ORDER BY namespace,name LIMIT 17", &[&stream_key(plan).0])?;
        if rows.len() > 16 {
            return Err(StoreError::Limit);
        }
        let mut result = Vec::new();
        for row in rows {
            let namespace: String = row.get(0);
            let revision =
                u32::try_from(row.get::<_, i64>(2)).map_err(|_| StoreError::ModelCorrupt)?;
            let model = installed::read(&mut tx, &namespace, row.get(1), revision)?;
            let fingerprint: String = row.get(3);
            if !plan.selects(&model.model.template) || fingerprint != model.sql.fingerprint {
                return Err(StoreError::ModelBinding);
            }
            result.push(BoundModel {
                namespace,
                model: model.model,
                fingerprint,
            });
        }
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(result)
    }

    /// The requested fingerprint must still be active. Numeric/bigint columns are cast to text
    /// in PostgreSQL, before JSON encoding, so JSON/JavaScript can never round their values.
    #[allow(clippy::too_many_arguments)]
    pub fn read_model(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ModelPage, StoreError> {
        self.read_model_query(
            plan,
            caller,
            lease,
            binding,
            &QueryOptions::default(),
            after,
            limit,
        )
    }

    /// Apply bounded typed predicates and native sorting after canonical visibility. Counts share
    /// the same filtered snapshot; pagination keys are bound to normalized query semantics.
    #[allow(clippy::too_many_arguments)]
    pub fn read_model_query(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        options: &QueryOptions,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ModelPage, StoreError> {
        let scope = plan.context().binding();
        let parties: Vec<_> = lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        if !(1..=25).contains(&limit)
            || after
                .is_some_and(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        {
            return Err(StoreError::Limit);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ModelBinding)?;
        let key = stream_key(plan).0;
        let row = tx.query_opt("SELECT revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key, &binding.namespace, &binding.model.name])?.ok_or(StoreError::ModelBinding)?;
        if row.get::<_, i64>(0) != i64::from(binding.model.revision)
            || row.get::<_, String>(1) != binding.fingerprint
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
        let validated = model
            .model
            .validate_query(options)
            .map_err(|_| StoreError::ModelQuery)?;
        let compiled = QueryPlan::new(&model.model, validated, after)?;
        let fields = model
            .model
            .sql_columns()
            .map_err(|_| StoreError::ModelCorrupt)?
            .into_iter()
            .skip(2)
            .map(|c| {
                // Field names come from the strict compiler; bool is the only JSON native number-like type.
                format!(
                    "'{}',m.\"{}\"{}",
                    c.name,
                    c.name,
                    if c.sql_type == "boolean" {
                        ""
                    } else {
                        "::text"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let template =
            serde_json::to_value(&model.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        let from = format!(
            "FROM {} m JOIN cantongrid.contracts c ON c.stream_key=m.\"_cg_stream_key\" AND c.contract_id=m.\"_cg_contract_id\" WHERE c.stream_key=$1 AND c.active AND c.witnesses && $2::text[] AND c.template=$3",
            model.relation()
        );
        // Count excludes keyset predicates but includes every caller-visible filter.
        let mut parameters: Vec<&(dyn ToSql + Sync)> = vec![&key, &parties, &template];
        parameters.extend(
            compiled.parameters[..compiled.filter_parameters]
                .iter()
                .map(|s| s as &(dyn ToSql + Sync)),
        );
        let total: i64 = tx
            .query_one(
                &format!("SELECT count(*) {from}{}", compiled.filters),
                &parameters,
            )?
            .get(0);
        parameters.truncate(3);
        parameters.extend(compiled.parameters.iter().map(|s| s as &(dyn ToSql + Sync)));
        let fetch = limit as i64 + 1;
        parameters.push(&fetch);
        let rows = tx.query(&format!("SELECT jsonb_build_object('contractId',c.contract_id,'values',jsonb_build_object({fields})) {from}{}{} ORDER BY {} LIMIT ${}", compiled.filters, compiled.page, compiled.order, parameters.len()), &parameters)?;
        let more = rows.len() > limit;
        let mut items = Vec::new();
        let mut last = None;
        let mut bytes = 0;
        for row in rows.into_iter().take(limit) {
            let value: serde_json::Value = row.get(0);
            bytes += serde_json::to_vec(&value)
                .map_err(|_| StoreError::ModelCorrupt)?
                .len();
            if bytes > 2 * 1024 * 1024 {
                return Err(StoreError::Limit);
            }
            if more && items.len() + 1 == limit {
                last = Some(compiled.key(&value)?);
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
