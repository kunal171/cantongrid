//! Real PostgreSQL transaction/recovery tests, isolated by generated synthetic source contexts.
use super::*;
use crate::journal::stream_key;
use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    config::Plan,
    fixture::Fixture,
    lifecycle::{Action, Batch},
    value::Value,
    visibility::{CallerGrants, PartyId},
};
use cantongrid_postgres::connect_env;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Scope-local cleanup never touches another test or an application/source database.
struct Scenario {
    plan: Plan,
    batches: Vec<Batch>,
}
impl Scenario {
    fn new() -> Self {
        initialize_env(VARIABLE).unwrap();
        let text = include_str!("../../../examples/fixtures/visibility/cantongrid.json");
        let original = Plan::parse(text).unwrap();
        let fixture = Fixture::parse(
            include_str!("../../../examples/fixtures/visibility/events.json"),
            &original,
        )
        .unwrap();
        let mut raw: serde_json::Value = serde_json::from_str(text).unwrap();
        raw["source"]["id"] = format!(
            "pg-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
        .into();
        let plan = Plan::parse(&raw.to_string()).unwrap();
        let batches = fixture
            .batches()
            .iter()
            .cloned()
            .map(|mut b| {
                b.context = plan.context().clone();
                b
            })
            .collect();
        Self { plan, batches }
    }
    fn open(&self) -> Store {
        Store::open_env(VARIABLE).unwrap()
    }
}
impl Drop for Scenario {
    fn drop(&mut self) {
        // No panic in cleanup can hide the original test failure.
        if let Ok(mut client) = connect_env(VARIABLE)
            && let Ok(mut tx) = client.transaction()
        {
            let key = stream_key(&self.plan).0;
            for table in ["outbox", "events", "contracts", "batches", "streams"] {
                if tx
                    .execute(
                        &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                        &[&key],
                    )
                    .is_err()
                {
                    return;
                }
            }
            let _ = tx.commit();
        }
    }
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn reopen_replay_preserves_exact_values_scope_history_and_outbox() {
    let scenario = Scenario::new();
    let mut store = scenario.open();
    assert!(store.apply(&scenario.plan, &scenario.batches[0]).unwrap());
    drop(store); // Recovery must not rely on any process-local engine.
    let mut store = scenario.open();
    assert!(store.apply(&scenario.plan, &scenario.batches[1]).unwrap());
    for batch in &scenario.batches {
        assert!(!store.apply(&scenario.plan, batch).unwrap());
    }
    assert_eq!(
        store.stats(&scenario.plan).unwrap(),
        Stats {
            batches: 2,
            contracts: 4,
            active_contracts: 3,
            events: 5,
            outbox: 5
        }
    );
    let engine = store.restore(&scenario.plan).unwrap();
    assert_eq!(engine.checkpoint(), Some("fixture-a"));
    let now = Instant::now();
    let parties = vec![PartyId::new("A").unwrap(), PartyId::new("B").unwrap()];
    let mut lease = SourceLease::from_verified(
        scenario.plan.context().binding(),
        parties,
        now,
        MAX_SOURCE_LEASE,
        now + MAX_SOURCE_LEASE,
    )
    .unwrap();
    for (names, active, history) in [
        (vec!["A"], 2, 4),
        (vec!["B"], 2, 2),
        (vec!["A", "B"], 3, 5),
        (vec![], 0, 0),
    ] {
        let caller = CallerGrants::from_verified(
            scenario.plan.context().binding(),
            names.into_iter().map(|p| PartyId::new(p).unwrap()),
        );
        assert_eq!(engine.active(&caller, &lease, now).len(), active);
        assert_eq!(engine.history(&caller, &lease, now).len(), history);
    }
    lease.invalidate();
    let caller = CallerGrants::from_verified(
        scenario.plan.context().binding(),
        [PartyId::new("A").unwrap()],
    );
    assert!(engine.active(&caller, &lease, now).is_empty());
    let row=store.client.query_one("SELECT payload #>> '{value,amount,value,decimal}',payload #>> '{value,sequence,value}' FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id='a2'", &[&stream_key(&scenario.plan).0]).unwrap();
    assert_eq!(row.get::<_, String>(0), "12345678901234567890.1234567891");
    assert_eq!(row.get::<_, String>(1), "9223372036854775807");
    let clauses: serde_json::Value=store.client.query_one("SELECT visibility_clauses FROM cantongrid.events WHERE stream_key=$1 AND event_id='archive-a'", &[&stream_key(&scenario.plan).0]).unwrap().get(0);
    assert_eq!(clauses, serde_json::json!([["A"], ["A"]]));
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn sql_failure_before_commit_rolls_back_all_state_progress_and_outbox() {
    let scenario = Scenario::new();
    let mut store = scenario.open();
    store.apply(&scenario.plan, &scenario.batches[0]).unwrap();
    let before = store.stats(&scenario.plan).unwrap();
    assert!(
        store
            .apply_inner(&scenario.plan, &scenario.batches[1], true)
            .is_err()
    );
    drop(store);
    let mut store = scenario.open();
    assert_eq!(store.stats(&scenario.plan).unwrap(), before);
    assert_eq!(
        store.restore(&scenario.plan).unwrap().checkpoint(),
        Some("fixture-z")
    );
    assert!(store.apply(&scenario.plan, &scenario.batches[1]).unwrap());
    assert!(!store.apply(&scenario.plan, &scenario.batches[1]).unwrap());
    assert_eq!(store.stats(&scenario.plan).unwrap().outbox, 5);
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn concurrent_worker_is_excluded_until_the_previous_transaction_ends() {
    let scenario = Scenario::new();
    let mut owner = scenario.open();
    let mut second = scenario.open();
    let mut held = owner.client.transaction().unwrap();
    held.query_one(
        "SELECT pg_advisory_xact_lock($1)",
        &[&stream_key(&scenario.plan).1],
    )
    .unwrap();
    assert_eq!(
        second.apply(&scenario.plan, &scenario.batches[0]),
        Err(StoreError::Busy)
    );
    held.rollback().unwrap();
    assert!(second.apply(&scenario.plan, &scenario.batches[0]).unwrap());
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn selection_drift_conflicts_and_bad_payloads_never_advance_the_checkpoint() {
    let scenario = Scenario::new();
    let mut store = scenario.open();
    store.apply(&scenario.plan, &scenario.batches[0]).unwrap();
    let before = store.stats(&scenario.plan).unwrap();
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/fixtures/visibility/cantongrid.json"
    ))
    .unwrap();
    raw["source"]["id"] = scenario.plan.context().identity().0.into();
    raw["source"]["parties"] = serde_json::json!(["A"]);
    let changed = Plan::parse(&raw.to_string()).unwrap();
    assert!(matches!(
        store.restore(&changed),
        Err(StoreError::PlanChanged)
    ));
    let mut bad = scenario.batches[1].clone();
    bad.events[1].action = Action::Create(Value::Int64("9223372036854775808".into()));
    assert_eq!(
        store.apply(&scenario.plan, &bad),
        Err(StoreError::InvalidBatch)
    );
    bad = scenario.batches[1].clone();
    bad.position = scenario.batches[0].position.clone();
    assert_eq!(
        store.apply(&scenario.plan, &bad),
        Err(StoreError::InvalidBatch)
    );
    assert_eq!(store.stats(&scenario.plan).unwrap(), before);
    // A different source generation is a distinct stream; it cannot adopt the old checkpoint.
    raw["source"]["parties"] = serde_json::json!(["A", "B"]);
    raw["source"]["generation"] = 2.into();
    let next = Plan::parse(&raw.to_string()).unwrap();
    assert!(store.restore(&next).unwrap().checkpoint().is_none());
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn inconsistent_checkpoint_and_oversized_delivery_fail_closed() {
    let scenario = Scenario::new();
    let mut store = scenario.open();
    let mut too_large = scenario.batches[0].clone();
    too_large.events[0].action = Action::Create(Value::Text("x".repeat(1024 * 1024)));
    assert_eq!(
        store.apply(&scenario.plan, &too_large),
        Err(StoreError::Limit)
    );
    assert_eq!(store.stats(&scenario.plan).unwrap().batches, 0);
    store.apply(&scenario.plan, &scenario.batches[0]).unwrap();
    let key = stream_key(&scenario.plan).0;
    store
        .client
        .execute(
            "UPDATE cantongrid.streams SET checkpoint='synthetic-corruption' WHERE stream_key=$1",
            &[&key],
        )
        .unwrap();
    assert!(matches!(
        store.restore(&scenario.plan),
        Err(StoreError::CorruptJournal)
    ));
    assert_eq!(
        store.apply(&scenario.plan, &scenario.batches[1]),
        Err(StoreError::CorruptJournal)
    );
}
