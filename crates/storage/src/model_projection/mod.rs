//! Private current-state projections. Canonical lifecycle, model rows and progress share a commit.
mod aggregate;
mod joins;
mod relationships;
pub use relationships::RelationshipQuery;
mod binding;
pub(crate) mod installed;
mod query;
mod query_plan;
pub(crate) mod rebuild;
pub use query::{BoundModel, ModelPage};
pub use rebuild::{RebuildAction, RebuildStatus};

use crate::{StoreError, journal::stream_key};
pub use binding::ModelBinding;
use cantongrid_core::{
    authorization::SourceLease,
    config::{Plan, TemplateId},
};
use cantongrid_postgres::postgres::Transaction;
use installed::Installed;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

const MAX_BINDINGS: usize = 16;
const MAX_WORK_BYTES: usize = 8 * 1024 * 1024;

/// One batch or atomic backfill shares this budget across all models, including filtered inputs.
/// Example: two models evaluating a 4 KiB payload consume at least 8 KiB, before output JSON.
#[derive(Default)]
pub(crate) struct Budget(usize);
impl Budget {
    fn charge(&mut self, bytes: usize) -> Result<(), StoreError> {
        self.0 = self.0.checked_add(bytes).ok_or(StoreError::Limit)?;
        if self.0 > MAX_WORK_BYTES {
            return Err(StoreError::Limit);
        }
        Ok(())
    }
}

/// Require the exact configured party set, rather than treating a broader lease as permission.
pub(crate) fn authorized(plan: &Plan, lease: &SourceLease) -> bool {
    let expected: BTreeSet<_> = plan.source_parties().cloned().collect();
    lease
        .source_parties(&plan.context().binding(), Instant::now())
        .is_some_and(|p| p == &expected)
}

/// Acquire the canonical writer lock before reading a prefix or changing any projection binding.
pub(crate) fn lock(tx: &mut Transaction<'_>, plan: &Plan) -> Result<(), StoreError> {
    if !tx
        .query_one(
            "SELECT pg_try_advisory_xact_lock($1)",
            &[&stream_key(plan).1],
        )?
        .get::<_, bool>(0)
    {
        return Err(StoreError::Busy);
    }
    Ok(())
}

/// Validate every alias before deduplicating shared physical tables, so a bad alias cannot hide.
pub(crate) fn load_bound(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    revision: i64,
) -> Result<Vec<Installed>, StoreError> {
    let rows = tx.query(
        "SELECT namespace,name,revision,fingerprint,bound_at_revision FROM cantongrid.model_bindings
         WHERE stream_key=$1 ORDER BY fingerprint,namespace,name LIMIT 17",
        &[&stream_key(plan).0],
    )?;
    if rows.len() > MAX_BINDINGS {
        return Err(StoreError::Limit);
    }
    let mut models = BTreeMap::new();
    for row in rows {
        let revision_number =
            u32::try_from(row.get::<_, i64>(2)).map_err(|_| StoreError::ModelCorrupt)?;
        let model = installed::load(tx, row.get(0), row.get(1), revision_number)?;
        if !plan.selects(&model.model.template)
            || row.get::<_, String>(3) != model.sql.fingerprint
            || row.get::<_, i64>(4) > revision
        {
            return Err(StoreError::ModelBinding);
        }
        models.insert(model.sql.fingerprint.clone(), model);
    }
    Ok(models.into_values().collect())
}

/// Canonical contract identity and raw source payload, already scope-checked by ingestion.
pub(crate) struct Input<'a> {
    pub template: &'a TemplateId,
    pub stream: &'a str,
    pub contract: &'a str,
    pub payload: &'a str,
    pub archive: bool,
}

/// Evaluate and insert one current contract, or verify/delete its previous row on archive.
/// Re-evaluating archives detects missing/unexpected filtered rows instead of hiding drift.
pub(crate) fn apply(
    tx: &mut Transaction<'_>,
    models: &[Installed],
    input: Input<'_>,
    budget: &mut Budget,
) -> Result<(), StoreError> {
    let Input {
        template,
        stream,
        contract,
        payload,
        archive,
    } = input;
    for model in models.iter().filter(|m| &m.model.template == template) {
        budget.charge(payload.len())?;
        let fields = model
            .model
            .evaluate(payload)
            .map_err(|_| StoreError::ModelProjection)?;
        let row = fields
            .as_ref()
            .map(|fields| {
                model
                    .model
                    .row_json(stream, contract, fields)
                    .map_err(|_| StoreError::ModelProjection)
            })
            .transpose()?;
        if let Some(row) = &row {
            budget.charge(
                serde_json::to_vec(row)
                    .map_err(|_| StoreError::ModelProjection)?
                    .len(),
            )?;
        }
        if archive {
            let count = tx.execute(
                &format!(
                    "DELETE FROM {} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2",
                    model.relation()
                ),
                &[&stream, &contract],
            )?;
            if count != u64::from(row.is_some()) {
                return Err(StoreError::ModelCorrupt);
            }
        } else if let Some(row) = row {
            let relation = model.relation();
            tx.execute(&format!("INSERT INTO {relation} SELECT * FROM jsonb_populate_record(NULL::{relation},$1)"), &[&row])?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
