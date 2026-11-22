//! Count-free restricted source observations for the local operator inspector.
use crate::{InspectError, validate_source};
use cantongrid_core::{authorization::SourceLease, config::Plan};
use cantongrid_postgres::{connect_env, postgres::IsolationLevel};
use std::{collections::BTreeSet, time::Instant};

/// Read temporal bounds only, under current exact source authority. No contract/event scans occur.
pub fn bounds_env(
    variable: &str,
    plan: &Plan,
    lease: &SourceLease,
) -> Result<serde_json::Value, InspectError> {
    let allowed = || {
        lease
            .source_parties(&plan.context().binding(), Instant::now())
            .is_some_and(|p| p == &plan.source_parties().cloned().collect::<BTreeSet<_>>())
    };
    if !allowed() {
        return Err(InspectError::Authorization);
    }
    let mut client = connect_env(variable)?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    validate_source(&mut tx)?;
    let row = tx.query_one(
        "SELECT public.oldest_offset(),public.latest_offset(),public.pruned_offset()",
        &[],
    )?;
    let oldest: Option<i64> = row.get(0);
    let latest: Option<i64> = row.get(1);
    let pruned: Option<i64> = row.get(2);
    if !matches!((oldest, latest), (None, None))
        && !matches!((oldest,latest),(Some(a),Some(b)) if a>=0 && b>=a)
    {
        return Err(InspectError::Bounds);
    }
    if pruned.is_some_and(|p| p < 0 || latest.is_some_and(|l| p > l)) {
        return Err(InspectError::Bounds);
    }
    tx.commit()?;
    if !allowed() {
        return Err(InspectError::Authorization);
    }
    Ok(
        serde_json::json!({"schema":"041","oldest":oldest.map(|v|v.to_string()),"latest":latest.map(|v|v.to_string()),"pruned":pruned.map(|v|v.to_string()),"historyComplete":false}),
    )
}
