//! Resolve declared current-state links; this module never writes or upgrades source-proof edges.
use super::{BoundModel, ModelPage, installed};
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::{IsolationLevel, Transaction};
use std::time::Instant;

/// Backend inputs are pinned bindings and a declaration name, never request-supplied key policies.
pub struct RelationshipQuery<'a> {
    pub source: &'a BoundModel,
    pub target: &'a BoundModel,
    pub name: &'a str,
    pub root: &'a str,
    pub after: Option<&'a str>,
    pub limit: usize,
}

/// Revalidate registry, physical schema, active binding and selected template in the read snapshot.
pub(crate) fn active(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    binding: &BoundModel,
) -> Result<installed::Installed, StoreError> {
    let row = tx.query_opt(
        "SELECT revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3",
        &[&stream_key(plan).0, &binding.namespace, &binding.model.name],
    )?.ok_or(StoreError::ModelBinding)?;
    let installed = installed::read(
        tx,
        &binding.namespace,
        &binding.model.name,
        binding.model.revision,
    )?;
    if row.get::<_, i64>(0) != i64::from(binding.model.revision)
        || row.get::<_, String>(1) != binding.fingerprint
        || installed.sql.fingerprint != binding.fingerprint
        || installed.model != binding.model
        || !plan.selects(&installed.model.template)
    {
        return Err(StoreError::ModelBinding);
    }
    Ok(installed)
}

impl Store {
    /// Read one outgoing hop under independent endpoint policies. Counts/fanout never see hidden
    /// targets. Canonical lifecycle and projections already share one atomic ingestion commit.
    pub fn read_relationship(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        query: RelationshipQuery<'_>,
    ) -> Result<ModelPage, StoreError> {
        let RelationshipQuery {
            source,
            target,
            name,
            root,
            after,
            limit,
        } = query;
        let valid_id = |s: &str| {
            !s.is_empty() && s.len() <= 1024 && s.trim() == s && !s.chars().any(char::is_control)
        };
        if !(1..=25).contains(&limit) || !valid_id(root) || after.is_some_and(|s| !valid_id(s)) {
            return Err(StoreError::ModelQuery);
        }
        if source.namespace != target.namespace || !super::authorized(plan, lease) {
            return Err(StoreError::Authorization);
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
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ModelBinding)?;
        let left = active(&mut tx, plan, source)?;
        let right = active(&mut tx, plan, target)?;
        left.model
            .validate_relationship(name, &right.model)
            .map_err(|_| StoreError::ModelQuery)?;
        let link = left
            .model
            .relationship(name)
            .map_err(|_| StoreError::ModelQuery)?;
        let stream = stream_key(plan).0;
        let left_template =
            serde_json::to_value(&left.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        let right_template =
            serde_json::to_value(&right.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        // Returning the same error for absent, archived, filtered and hidden roots avoids an oracle.
        let root_from = format!(
            "FROM {} s JOIN cantongrid.contracts c ON c.stream_key=s.\"_cg_stream_key\" AND c.contract_id=s.\"_cg_contract_id\" WHERE c.stream_key=$1 AND c.contract_id=$3 AND c.active AND c.witnesses && $2::text[] AND c.template=$4",
            left.relation()
        );
        if tx
            .query_opt(
                &format!("SELECT 1 {root_from}"),
                &[&stream, &parties, &root, &left_template],
            )?
            .is_none()
        {
            return Err(StoreError::RelationshipUnavailable);
        }
        // Only compiler-validated identifiers are interpolated; all values remain bound parameters.
        // LIMIT max+1 bounds returned IDs and signals overflow of *visible* fanout only.
        let sql = format!(
            "SELECT c.contract_id FROM {} t JOIN cantongrid.contracts c ON c.stream_key=t.\"_cg_stream_key\" AND c.contract_id=t.\"_cg_contract_id\" WHERE c.stream_key=$1 AND c.active AND c.witnesses && $2::text[] AND c.template=$5 AND c.contract_id<>$3 AND t.\"{}\"=(SELECT s.\"{}\" {root_from}) ORDER BY c.contract_id COLLATE \"C\" LIMIT $6",
            right.relation(),
            link.target.field,
            link.source_field
        );
        let rows = tx.query(
            &sql,
            &[
                &stream,
                &parties,
                &root,
                &left_template,
                &right_template,
                &(i64::from(link.max_targets) + 1),
            ],
        )?;
        if rows.len() > link.max_targets as usize {
            return Err(StoreError::Limit);
        }
        let total = rows.len().to_string();
        let mut ids = rows
            .into_iter()
            .map(|r| r.get::<_, String>(0))
            .filter(|id| after.is_none_or(|a| id.as_str() > a))
            .collect::<Vec<_>>();
        // Use byte-order SQL below so Rust's keyset comparison has identical collation semantics.
        let more = ids.len() > limit;
        ids.truncate(limit);
        let next_after = if more { ids.last().cloned() } else { None };
        let items = ids.into_iter().map(|id|serde_json::json!({
            "sourceContractId":root,"targetContractId":id,"relationship":"declared",
            "declaration":name,"declarationRevision":link.revision.to_string(),
            "sourceModelRevision":source.model.revision.to_string(),"targetModelRevision":target.model.revision.to_string(),
            "sourceFingerprint":source.fingerprint,"targetFingerprint":target.fingerprint,
            "coverage":"current_active_projection","depth":1
        })).collect();
        tx.commit()?;
        lease
            .read_parties(&scope, caller, Instant::now())
            .ok_or(StoreError::Authorization)?;
        Ok(ModelPage {
            items,
            visible_total: total,
            next_after,
            fingerprint: source.fingerprint.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
