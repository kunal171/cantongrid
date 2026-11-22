//! Bounded temporal checks use owned PostgreSQL and synthetic source authority, never ledger writes.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, archive, lease};
use cantongrid_core::{
    lifecycle::{Batch, Origin},
    visibility::PartyId,
};

/// Encode the exact supported source cursor; final PK acknowledges every same-offset tie.
fn position(boundary: i64) -> String {
    serde_json::json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string()
}
/// Explicitly distinguish a snapshot observation from a later ledger transition.
fn origin(batch: &mut Batch, offset: i64, snapshot: bool) {
    for event in &mut batch.events {
        event.origin = Some(Origin {
            offset,
            snapshot_observation: snapshot,
        });
    }
}
/// Current caller grants are independent of the old creation's observation time.
fn caller(plan: &Plan, parties: &[&str]) -> CallerGrants {
    CallerGrants::from_verified(
        plan.context().binding(),
        parties.iter().map(|p| PartyId::new(*p).unwrap()),
    )
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn temporal_membership_exact_values_visibility_replay_and_projection_rebuild_agree() {
    let mut s = Scenario::visibility();
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [s.models[0].template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    let plan = s.plans[0].clone();
    let authority = || lease(&plan, vec!["A", "B"]);
    let mut initial = s.create(&position(10), "first", PAYLOAD);
    origin(&mut initial, 3, true);
    let mut hidden = s.create(&position(10), "hidden", PAYLOAD);
    origin(&mut hidden, 4, true);
    hidden.events[0].witnesses = [PartyId::new("B").unwrap()].into();
    initial.events.extend(hidden.events);
    let mut store = s.open();
    store
        .apply_authorized(&plan, &initial, &authority())
        .unwrap();
    let mut later = archive(&s, &position(20), "first");
    origin(&mut later, 15, false);
    let mut recreated = s.create(&position(20), "second", PAYLOAD);
    origin(&mut recreated, 15, false);
    let mut third = s.create(&position(20), "third", PAYLOAD);
    origin(&mut third, 19, false);
    later.events.extend(recreated.events);
    later.events.extend(third.events);
    store.apply_authorized(&plan, &later, &authority()).unwrap();
    let a = caller(&plan, &["A"]);
    let b = caller(&plan, &["B"]);
    let read = |store: &mut Store, id: &CallerGrants, offset, after| {
        store.read_as_of(
            &plan,
            id,
            &authority(),
            TemporalQuery {
                offset,
                after,
                limit: 1,
            },
        )
    };
    let before = read(&mut store, &a, 10, None).unwrap();
    assert_eq!(before.visible_total, 1);
    assert_eq!(before.items[0]["contractId"], "first");
    assert!(
        serde_json::to_string(&before.items)
            .unwrap()
            .contains("12345678901234567890")
    );
    assert_eq!(
        read(&mut store, &b, 15, None).unwrap().items[0]["contractId"],
        "hidden"
    );
    assert_eq!(
        read(&mut store, &a, 14, None).unwrap().items[0]["contractId"],
        "first"
    );
    assert_eq!(
        read(&mut store, &a, 15, None).unwrap().items[0]["contractId"],
        "second"
    );
    let first = read(&mut store, &a, 20, None).unwrap();
    assert_eq!(first.visible_total, 2);
    assert_eq!(
        read(&mut store, &a, 20, first.next_after.as_deref())
            .unwrap()
            .items[0]["contractId"],
        "third"
    );
    assert!(!store.apply_authorized(&plan, &later, &authority()).unwrap());
    store
        .bind_model(
            &plan,
            &authority(),
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
    let rebuilt = read(&mut s.open(), &a, 10, None).unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&rebuilt).unwrap()
    );
    for offset in [9, 21] {
        assert_eq!(
            read(&mut store, &a, offset, None).err(),
            Some(StoreError::TemporalUnavailable)
        );
    }
    let mut revoked = authority();
    revoked.invalidate();
    assert_eq!(
        store
            .read_as_of(
                &plan,
                &a,
                &revoked,
                TemporalQuery {
                    offset: 10,
                    after: None,
                    limit: 1
                }
            )
            .err(),
        Some(StoreError::Authorization)
    );
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn temporal_incomplete_snapshot_and_unacknowledged_interval_never_look_empty() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let mut store = s.open();
    let initial =
        serde_json::json!({"snapshot":true,"boundary":10,"through":10,"offset":10,"pk":1})
            .to_string();
    let mut batch = s.create(&initial, "first", PAYLOAD);
    origin(&mut batch, 2, true);
    store.apply_authorized(plan, &batch, &s.lease()).unwrap();
    let user = caller(plan, &["A"]);
    let query = |store: &mut Store, offset| {
        store.read_as_of(
            plan,
            &user,
            &s.lease(),
            TemporalQuery {
                offset,
                after: None,
                limit: 25,
            },
        )
    };
    assert_eq!(
        query(&mut store, 10).err(),
        Some(StoreError::TemporalUnavailable)
    );
    let final_snapshot = Batch {
        context: plan.context().clone(),
        position: position(10),
        events: vec![],
    };
    store
        .apply_authorized(plan, &final_snapshot, &s.lease())
        .unwrap();
    assert_eq!(query(&mut store, 10).unwrap().visible_total, 1);
    let partial =
        serde_json::json!({"snapshot":false,"boundary":10,"through":20,"offset":15,"pk":1})
            .to_string();
    let mut forward = s.create(&partial, "second", PAYLOAD);
    origin(&mut forward, 15, false);
    store.apply_authorized(plan, &forward, &s.lease()).unwrap();
    assert_eq!(
        query(&mut store, 15).err(),
        Some(StoreError::TemporalUnavailable)
    );
    assert_eq!(query(&mut store, 10).unwrap().visible_total, 1);
    store
        .apply_authorized(
            plan,
            &Batch {
                context: plan.context().clone(),
                position: position(20),
                events: vec![],
            },
            &s.lease(),
        )
        .unwrap();
    assert_eq!(query(&mut store, 15).unwrap().visible_total, 2);
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn temporal_missing_journal_rows_and_origins_fail_closed_without_checkpoint_mutation() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let mut store = s.open();
    let mut batch = s.create(&position(10), "first", PAYLOAD);
    origin(&mut batch, 2, true);
    store.apply_authorized(plan, &batch, &s.lease()).unwrap();
    store
        .apply_authorized(
            plan,
            &Batch {
                context: plan.context().clone(),
                position: position(20),
                events: vec![],
            },
            &s.lease(),
        )
        .unwrap();
    let original = store.checkpoint(plan).unwrap();
    let key = stream_key(plan).0;
    store.client.execute("UPDATE cantongrid.events SET envelope=jsonb_set(envelope,'{events,0,origin}','null') WHERE stream_key=$1",&[&key]).unwrap();
    assert_eq!(
        store
            .read_as_of(
                plan,
                &caller(plan, &["A"]),
                &s.lease(),
                TemporalQuery {
                    offset: 10,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::TemporalUnavailable)
    );
    assert_eq!(store.checkpoint(plan).unwrap(), original);
    store
        .client
        .execute(
            "DELETE FROM cantongrid.batches WHERE stream_key=$1 AND revision=1",
            &[&key],
        )
        .unwrap();
    assert_eq!(
        store
            .read_as_of(
                plan,
                &caller(plan, &["A"]),
                &s.lease(),
                TemporalQuery {
                    offset: 10,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::TemporalUnavailable)
    );
}
