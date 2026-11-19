//! Activation changes metadata only. Retirement cleanup is a later bounded step.
use super::*;

/// Compare the captured old binding (or the already activated target), including its fingerprint.
pub(super) fn binding(
    tx: &mut Transaction<'_>,
    key: &str,
    job: &Job,
    activated: bool,
) -> Result<(), StoreError> {
    let row=tx.query_opt("SELECT revision,fingerprint FROM cantongrid.model_bindings WHERE stream_key=$1 AND namespace=$2 AND name=$3", &[&key,&job.namespace,&job.name])?;
    let actual = row.map(|r| (r.get::<_, i64>(0), r.get::<_, String>(1)));
    let expected = if activated {
        Some((i64::from(job.revision), job.fingerprint.clone()))
    } else {
        job.previous
            .zip(job.previous_fingerprint.clone())
            .map(|(r, f)| (i64::from(r), f))
    };
    if actual != expected {
        return Err(StoreError::ModelRebuild);
    }
    if !activated && tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_bindings WHERE stream_key=$1 AND fingerprint=$2)", &[&key,&job.fingerprint])?.get::<_,bool>(0) { return Err(StoreError::ModelRebuild); }
    Ok(())
}

/// Promote only a fully reconciled target and leave row retirement to bounded steps.
pub(super) fn activate(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    key: &str,
    job: &mut Job,
    revision: i64,
) -> Result<(), StoreError> {
    if matches!(job.phase.as_str(), "cleanup" | "complete") {
        return binding(tx, key, job, true);
    }
    if job.phase != "ready" || pending(tx, key, &job.id)? {
        return Err(StoreError::ModelRebuild);
    }
    binding(tx, key, job, false)?;
    load_bound(tx, plan, revision)?;
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM cantongrid.model_bindings WHERE stream_key=$1",
            &[&key],
        )?
        .get(0);
    if job.previous.is_none() && count >= MAX_BINDINGS as i64 {
        return Err(StoreError::Limit);
    }
    tx.execute("INSERT INTO cantongrid.model_bindings(stream_key,namespace,name,revision,fingerprint,bound_at_revision) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(stream_key,namespace,name) DO UPDATE SET revision=EXCLUDED.revision,fingerprint=EXCLUDED.fingerprint,bound_at_revision=EXCLUDED.bound_at_revision,bound_at=now()", &[&key,&job.namespace,&job.name,&i64::from(job.revision),&job.fingerprint,&revision])?;
    // Never DELETE the previous revision in this transaction: it can contain millions of rows.
    job.phase = if job.previous.is_some() {
        "cleanup"
    } else {
        "complete"
    }
    .into();
    Ok(())
}

/// Cancellation retains its reservation until bounded target and queue cleanup finishes.
pub(super) fn cancel(job: &mut Job) -> Result<(), StoreError> {
    match job.phase.as_str() {
        "cleaning" | "backfill" | "catchup" | "ready" | "cancelling" => {
            job.phase = "cancelling".into()
        }
        "cancelled" => {}
        _ => return Err(StoreError::ModelRebuild),
    }
    Ok(())
}
