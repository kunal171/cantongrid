//! Each step has bounded rows/work and commits its progress with its output changes.
use super::*;
use crate::model_projection::Budget;
use cantongrid_core::value::Value;

/// Delete one bounded keyset from a compiler-owned relation. The caller protects its reservation.
fn delete(
    tx: &mut Transaction<'_>,
    relation: &str,
    key: &str,
    limit: i64,
) -> Result<u64, StoreError> {
    Ok(tx.execute(&format!("DELETE FROM {relation} m USING (SELECT \"_cg_contract_id\" FROM {relation} WHERE \"_cg_stream_key\"=$1 ORDER BY \"_cg_contract_id\" LIMIT $2) d WHERE m.\"_cg_stream_key\"=$1 AND m.\"_cg_contract_id\"=d.\"_cg_contract_id\""), &[&key,&limit])?)
}

/// Reconcile immutable payload plus current active flag. An archived row is removed, never revived.
fn put(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    key: &str,
    job: &mut Job,
    target: &installed::Installed,
    row: &Row,
    budget: &mut Budget,
) -> Result<(), StoreError> {
    let contract: &str = row.get(0);
    let deleted = tx.execute(
        &format!(
            "DELETE FROM {} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2",
            target.relation()
        ),
        &[&key, &contract],
    )?;
    job.projected = job
        .projected
        .checked_sub(deleted as i64)
        .filter(|v| *v >= 0)
        .ok_or(StoreError::ModelRebuild)?;
    if !row.get::<_, bool>(3) {
        return Ok(());
    }
    let witnesses: Vec<String> = row.get(2);
    if !witnesses
        .iter()
        .any(|p| plan.source_parties().any(|e| e.as_str() == p))
    {
        return Err(StoreError::Authorization);
    }
    let Value::RawJson(payload) =
        serde_json::from_value(row.get(1)).map_err(|_| StoreError::ModelProjection)?
    else {
        return Err(StoreError::ModelProjection);
    };
    budget.charge(payload.len())?;
    let fields = target
        .model
        .evaluate(&payload)
        .map_err(|_| StoreError::ModelProjection)?;
    if let Some(fields) = fields {
        let output = target
            .model
            .row_json(key, contract, &fields)
            .map_err(|_| StoreError::ModelProjection)?;
        budget.charge(
            serde_json::to_vec(&output)
                .map_err(|_| StoreError::ModelProjection)?
                .len(),
        )?;
        let relation = target.relation();
        tx.execute(
            &format!(
                "INSERT INTO {relation} SELECT * FROM jsonb_populate_record(NULL::{relation},$1)"
            ),
            &[&output],
        )?;
        job.projected = job.projected.checked_add(1).ok_or(StoreError::Limit)?;
    }
    Ok(())
}

/// Advance one phase/page under the source lock; never combine scanning with activation.
pub(super) fn run(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    key: &str,
    job: &mut Job,
    target: &installed::Installed,
    limit: i64,
) -> Result<(), StoreError> {
    if matches!(job.phase.as_str(), "complete" | "cancelled") {
        return Ok(());
    }
    let activated = job.phase == "cleanup";
    super::transition::binding(tx, key, job, activated)?;
    match job.phase.as_str() {
        "cleaning" => {
            if delete(tx, &target.relation(), key, limit)? == 0 {
                job.phase = "backfill".into();
            }
        }
        "backfill" => {
            let template = serde_json::to_value(&target.model.template)
                .map_err(|_| StoreError::ModelRebuild)?;
            let rows=tx.query("SELECT contract_id,payload,witnesses,active FROM cantongrid.contracts WHERE stream_key=$1 AND template=$2 AND active AND ($3::text IS NULL OR contract_id>$3) ORDER BY contract_id LIMIT $4", &[&key,&template,&job.after,&limit])?;
            let done = rows.len() < limit as usize;
            let mut budget = Budget::default();
            for row in rows {
                put(tx, plan, key, job, target, &row, &mut budget)?;
                job.after = Some(row.get(0));
                job.scanned = job.scanned.checked_add(1).ok_or(StoreError::Limit)?;
            }
            if done {
                job.phase = "catchup".into();
            }
        }
        "catchup" | "ready" => {
            let rows=tx.query("SELECT contract_id FROM cantongrid.model_rebuild_dirty WHERE stream_key=$1 AND job_id=$2 ORDER BY contract_id LIMIT $3", &[&key,&job.id,&limit])?;
            let mut budget = Budget::default();
            for dirty in rows {
                let contract: &str = dirty.get(0);
                let row=tx.query_opt("SELECT contract_id,payload,witnesses,active,template FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2", &[&key,&contract])?.ok_or(StoreError::ModelRebuild)?;
                if row.get::<_, serde_json::Value>(4)
                    != serde_json::to_value(&target.model.template)
                        .map_err(|_| StoreError::ModelRebuild)?
                {
                    return Err(StoreError::ModelRebuild);
                }
                put(tx, plan, key, job, target, &row, &mut budget)?;
                tx.execute("DELETE FROM cantongrid.model_rebuild_dirty WHERE stream_key=$1 AND job_id=$2 AND contract_id=$3", &[&key,&job.id,&contract])?;
                job.processed = job.processed.checked_add(1).ok_or(StoreError::Limit)?;
            }
            job.phase = if pending(tx, key, &job.id)? {
                "catchup"
            } else {
                "ready"
            }
            .into();
        }
        "cancelling" => {
            let deleted = delete(tx, &target.relation(), key, limit)?;
            job.projected = job.projected.saturating_sub(deleted as i64).max(0);
            if deleted == 0 {
                tx.execute("DELETE FROM cantongrid.model_rebuild_dirty d USING (SELECT contract_id FROM cantongrid.model_rebuild_dirty WHERE stream_key=$1 AND job_id=$2 ORDER BY contract_id LIMIT $3) p WHERE d.stream_key=$1 AND d.job_id=$2 AND d.contract_id=p.contract_id", &[&key,&job.id,&limit])?;
                if !pending(tx, key, &job.id)? {
                    job.phase = "cancelled".into();
                    job.projected = 0;
                }
            }
        }
        "cleanup" => {
            let fp = job
                .previous_fingerprint
                .as_ref()
                .ok_or(StoreError::ModelRebuild)?;
            let referenced:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_bindings WHERE stream_key=$1 AND fingerprint=$2)", &[&key,&fp])?.get(0);
            if referenced {
                job.phase = "complete".into();
            } else {
                let previous = installed::load(
                    tx,
                    &job.namespace,
                    &job.name,
                    job.previous.ok_or(StoreError::ModelRebuild)?,
                )?;
                if previous.sql.fingerprint != *fp {
                    return Err(StoreError::ModelRebuild);
                }
                if delete(tx, &previous.relation(), key, limit)? == 0 {
                    job.phase = "complete".into();
                }
            }
        }
        _ => return Err(StoreError::ModelRebuild),
    }
    Ok(())
}
