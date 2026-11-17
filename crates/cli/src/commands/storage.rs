//! Explicit destination commands and fixture-only durable ingestion.
use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    fixture::Fixture,
    visibility::{CallerGrants, PartyId},
};
use cantongrid_storage::Store;
use std::{collections::BTreeSet, path::Path, time::Instant};

/// Migrations require an explicit command; opening or querying a source cannot run them.
pub fn initialize(variable: &str) -> Result<(), String> {
    cantongrid_storage::initialize_env(variable).map_err(|e| e.to_string())?;
    crate::print_json(
        &serde_json::json!({"operation":"destination_initialized","schema_version":1}),
    )
}

/// Index complete synthetic input batch-by-batch; query grants remain explicitly simulated.
pub fn fixture(action: &str, path: &Path, variable: &str, flags: &[String]) -> Result<(), String> {
    let plan = crate::load_plan(path)?;
    let mut parties = vec![];
    let mut remaining = flags.iter();
    while let Some(flag) = remaining.next() {
        if action != "query" || flag != "--party" {
            return Err("only fixture query accepts --party <synthetic-party>".into());
        }
        parties.push(remaining.next().ok_or("missing fixture party")?.clone());
    }
    let mut store = Store::open_env(variable).map_err(|e| e.to_string())?;
    match action {
        "index" => {
            let input = path
                .parent()
                .unwrap_or(Path::new("."))
                .join(plan.fixture_path());
            let fixture =
                Fixture::parse(&crate::read_document(&input)?, &plan).map_err(|e| e.to_string())?;
            let mut applied = 0;
            for batch in fixture.batches() {
                applied += usize::from(store.apply(&plan, batch).map_err(|e| e.to_string())?);
            }
            let stats = store.stats(&plan).map_err(|e| e.to_string())?;
            crate::print_json(
                &serde_json::json!({"mode":"fixture","storage":"postgres","operator_diagnostics":true,
                "applied_batches":applied,"result":stats}),
            )
        }
        "status" => {
            let stats = store.stats(&plan).map_err(|e| e.to_string())?;
            crate::print_json(
                &serde_json::json!({"mode":"fixture","storage":"postgres","operator_diagnostics":true,"result":stats}),
            )
        }
        "query" => {
            let caller: BTreeSet<_> = parties
                .into_iter()
                .map(PartyId::new)
                .collect::<Result<_, _>>()
                .map_err(|_| "invalid fixture caller parties")?;
            let source: BTreeSet<_> = plan.source_parties().cloned().collect();
            if !caller.is_subset(&source) {
                return Err(
                    "fixture caller parties must be within the declared source scope".into(),
                );
            }
            let engine = store.restore(&plan).map_err(|e| e.to_string())?;
            let now = Instant::now();
            // This command simulates a caller solely for synthetic fixtures. It does not authenticate.
            let lease = SourceLease::from_verified(
                plan.context().binding(),
                source,
                now,
                MAX_SOURCE_LEASE,
                now + MAX_SOURCE_LEASE,
            )
            .map_err(|e| e.to_string())?;
            let caller = CallerGrants::from_verified(plan.context().binding(), caller);
            crate::print_json(
                &serde_json::json!({"mode":"fixture","storage":"postgres","authentication":"simulated_fixture_grants",
                "visible_active":engine.active(&caller,&lease,now).len(),"visible_history":engine.history(&caller,&lease,now).len()}),
            )
        }
        _ => Err("unknown durable fixture operation".into()),
    }
}
