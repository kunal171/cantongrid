//! Owned-database controls for immutable baselines and concurrent post-baseline lifecycle delivery.
use super::*;
use crate::model_projection::tests::{archive, lease};
use cantongrid_core::lifecycle::{Batch, Origin};

/// Complete source cursor; all same-offset ties are acknowledged before anchoring a baseline.
fn position(boundary: i64) -> String {
    serde_json::json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string()
}
/// Controlled canonical batch with real temporal origins and exact financial payloads.
fn creates(s: &Scenario, boundary: i64, ids: &[&str], snapshot: bool) -> Batch {
    let mut batch = Batch {
        context: s.plans[0].context().clone(),
        position: position(boundary),
        events: vec![],
    };
    for id in ids {
        batch
            .events
            .extend(s.create(&batch.position, id, PAYLOAD).events);
    }
    for event in &mut batch.events {
        event.origin = Some(Origin {
            offset: boundary,
            snapshot_observation: snapshot,
        });
    }
    batch
}
/// A owns this fixture's source scope; separate caller identity still must be supplied for every operation.
fn caller(s: &Scenario) -> CallerGrants {
    CallerGrants::from_verified(s.plans[0].context().binding(), [PartyId::new("A").unwrap()])
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn bootstrap_pages_pin_state_while_new_connection_ingests_and_retries_do_not_recapture() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let a = caller(&s);
    let c = consumer();
    let mut store = s.open();
    store
        .apply_authorized(plan, &creates(&s, 10, &["a", "z"], true), &s.lease())
        .unwrap();
    let sub = store
        .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
        .unwrap();
    let query = |store: &mut crate::Store, after| {
        store.read_bootstrap(
            plan,
            &a,
            &s.lease(),
            &c,
            DeliveryQuery {
                subscription: &sub.id,
                after,
                limit: 1,
            },
        )
    };
    let first = query(&mut store, None).unwrap();
    assert_eq!(first.items[0]["contractId"], "a");
    assert_eq!(first.visible_total, 2);
    // Commit a replacement on a separate SQL connection between baseline pages.
    let mut later = archive(&s, &position(20), "z");
    later.events[0].origin = Some(Origin {
        offset: 20,
        snapshot_observation: false,
    });
    later.events.extend(creates(&s, 20, &["new"], false).events);
    std::thread::scope(|scope| {
        scope
            .spawn(|| s.open().apply_authorized(plan, &later, &s.lease()).unwrap())
            .join()
            .unwrap();
    });
    let retry = s
        .open()
        .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
        .unwrap();
    assert_eq!(sub.id, retry.id);
    assert_eq!(sub.receipt, retry.receipt);
    let last = query(&mut s.open(), first.next_after.as_deref()).unwrap();
    assert_eq!(last.items[0]["contractId"], "z");
    assert_eq!(last.visible_total, 2);
    let page = store
        .read_deliveries(
            plan,
            &a,
            &s.lease(),
            &c,
            DeliveryQuery {
                subscription: &sub.id,
                after: None,
                limit: 25,
            },
        )
        .unwrap();
    assert_eq!(
        page.events
            .iter()
            .map(|e| e["eventId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["archive-z", "create-new"]
    );
    assert!(
        serde_json::to_string(&first.items)
            .unwrap()
            .contains("12345678901234567890")
    );
    store
        .acknowledge(
            plan,
            &a,
            &s.lease(),
            &c,
            &sub.id,
            page.receipt.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
            .err(),
        Some(StoreError::BootstrapConflict)
    );
    assert_eq!(
        query(&mut store, None).err(),
        Some(StoreError::BootstrapConflict)
    );
    assert!(
        store
            .read_deliveries(
                plan,
                &a,
                &s.lease(),
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 25
                }
            )
            .unwrap()
            .events
            .is_empty()
    );
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn bootstrap_rejects_ordinary_names_wrong_owners_partial_intervals_missing_history_and_stale_ingestion()
 {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let a = caller(&s);
    let c = consumer();
    let mut store = s.open();
    store
        .apply_authorized(plan, &creates(&s, 10, &["a"], true), &s.lease())
        .unwrap();
    let ordinary = store
        .subscribe(
            plan,
            &a,
            &s.lease(),
            &c,
            SubscriptionRequest {
                name: "ordinary",
                start: DeliveryStart::Now,
            },
        )
        .unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "ordinary")
            .err(),
        Some(StoreError::BootstrapConflict)
    );
    let sub = store
        .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
        .unwrap();
    assert_eq!(
        store
            .subscribe(
                plan,
                &a,
                &s.lease(),
                &c,
                SubscriptionRequest {
                    name: "baseline",
                    start: DeliveryStart::Now
                }
            )
            .err(),
        Some(StoreError::BootstrapConflict)
    );
    let stranger = Consumer::from_verified("c".repeat(64), "d".repeat(64)).unwrap();
    assert_eq!(
        store
            .read_bootstrap(
                plan,
                &a,
                &s.lease(),
                &stranger,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::SubscriptionUnavailable)
    );
    assert_eq!(
        store
            .read_bootstrap(
                plan,
                &a,
                &s.lease(),
                &c,
                DeliveryQuery {
                    subscription: &ordinary.id,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::BootstrapConflict)
    );
    store.client.execute("UPDATE cantongrid.delivery_subscriptions SET expires_at=now()-interval '1 second' WHERE id=$1",&[&sub.id]).unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
            .err(),
        Some(StoreError::SubscriptionUnavailable)
    );
    let mut partial = creates(&s, 20, &["later"], false);
    partial.position =
        serde_json::json!({"snapshot":false,"boundary":10,"through":20,"offset":20,"pk":1})
            .to_string();
    store.apply_authorized(plan, &partial, &s.lease()).unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "partial")
            .err(),
        Some(StoreError::TemporalUnavailable)
    );
    store
        .apply_authorized(plan, &creates(&s, 20, &[], false), &s.lease())
        .unwrap();
    let key = stream_key(plan).0;
    store.client.execute("UPDATE cantongrid.streams SET observed_at=now()-interval '1 minute' WHERE stream_key=$1",&[&key]).unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "stale")
            .err(),
        Some(StoreError::DeliveryStale)
    );
    store.observe_authorized(plan, &s.lease()).unwrap();
    // Corrupt only this fixture: clear the dependent queue row before removing canonical evidence.
    store
        .client
        .execute(
            "DELETE FROM cantongrid.outbox WHERE stream_key=$1 AND event_id='create-a'",
            &[&key],
        )
        .unwrap();
    store
        .client
        .execute(
            "DELETE FROM cantongrid.events WHERE stream_key=$1 AND event_id='create-a'",
            &[&key],
        )
        .unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "missing")
            .err(),
        Some(StoreError::TemporalUnavailable)
    );
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn bootstrap_retention_loss_is_explicit_and_additive_baselines_cannot_rewrite_a_pinned_snapshot() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let a = caller(&s);
    let c = consumer();
    let mut store = s.open();
    store
        .apply_authorized(plan, &creates(&s, 10, &["first"], true), &s.lease())
        .unwrap();
    let sub = store
        .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
        .unwrap();
    store
        .apply_authorized(
            plan,
            &creates(&s, 20, &["second", "third"], false),
            &s.lease(),
        )
        .unwrap();
    store.prune_deliveries(plan, &s.lease(), 1, 256).unwrap();
    assert_eq!(
        store
            .read_bootstrap(
                plan,
                &a,
                &s.lease(),
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::DeliveryGap)
    );
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "baseline")
            .err(),
        Some(StoreError::DeliveryGap)
    );
    // A fresh pairing is explicit; raising the common floor afterwards invalidates its old baseline.
    let fresh = store
        .bootstrap_subscription(plan, &a, &s.lease(), &c, "fresh")
        .unwrap();
    store
        .apply_authorized(plan, &creates(&s, 30, &[], false), &s.lease())
        .unwrap();
    let manifest = serde_json::json!({"checkpoint":position(30)});
    store.client.execute("INSERT INTO cantongrid.selection_upgrades(stream_key,fingerprint,manifest) VALUES($1,'bootstrap-test-floor',$2)",&[&stream_key(plan).0,&manifest]).unwrap();
    assert_eq!(
        store
            .read_bootstrap(
                plan,
                &a,
                &s.lease(),
                &c,
                DeliveryQuery {
                    subscription: &fresh.id,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::TemporalUnavailable)
    );
    // The trusted lease must still cover the complete configured source scope.
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &lease(plan, vec!["B"]), &c, "revoked")
            .err(),
        Some(StoreError::Authorization)
    );
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn bootstrap_serializes_with_writer_and_applies_shared_registration_caps() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let a = caller(&s);
    let c = consumer();
    let mut store = s.open();
    store
        .apply_authorized(plan, &creates(&s, 10, &["first"], true), &s.lease())
        .unwrap();
    let mut writer = s.open();
    let mut tx = writer.client.transaction().unwrap();
    crate::model_projection::lock(&mut tx, plan).unwrap();
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "blocked")
            .err(),
        Some(StoreError::Busy)
    );
    tx.rollback().unwrap();
    for n in 0..16 {
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, &format!("baseline-{n}"))
            .unwrap();
    }
    assert_eq!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "over-budget")
            .err(),
        Some(StoreError::DeliveryBackpressure)
    );
    assert!(
        store
            .bootstrap_subscription(plan, &a, &s.lease(), &c, "../bad")
            .is_err()
    );
    // Ordinary and bootstrap registrations share one principal budget, rather than doubling it.
    assert_eq!(
        store
            .subscribe(
                plan,
                &a,
                &s.lease(),
                &c,
                SubscriptionRequest {
                    name: "ordinary",
                    start: DeliveryStart::Now
                }
            )
            .err(),
        Some(StoreError::DeliveryBackpressure)
    );
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn bootstrap_hidden_only_updates_and_pruning_do_not_change_visible_snapshot_or_create_a_gap() {
    let mut s = Scenario::visibility();
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [s.models[0].template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    let plan = &s.plans[0];
    let authority = || lease(plan, vec!["A", "B"]);
    let a = caller(&s);
    let c = consumer();
    let mut store = s.open();
    store
        .apply_authorized(plan, &creates(&s, 10, &["visible"], true), &authority())
        .unwrap();
    let sub = store
        .bootstrap_subscription(plan, &a, &authority(), &c, "baseline")
        .unwrap();
    let read = |store: &mut crate::Store| {
        store.read_bootstrap(
            plan,
            &a,
            &authority(),
            &c,
            DeliveryQuery {
                subscription: &sub.id,
                after: None,
                limit: 25,
            },
        )
    };
    let before = read(&mut store).unwrap();
    let mut hidden = creates(&s, 20, &["hidden-one", "hidden-two"], false);
    for e in &mut hidden.events {
        e.witnesses = [PartyId::new("B").unwrap()].into();
    }
    store.apply_authorized(plan, &hidden, &authority()).unwrap();
    assert_eq!(
        store.prune_deliveries(plan, &authority(), 1, 256).unwrap(),
        2
    );
    assert_eq!(read(&mut store).unwrap().items, before.items);
    let retry = store
        .bootstrap_subscription(plan, &a, &authority(), &c, "baseline")
        .unwrap();
    assert_eq!(sub.receipt, retry.receipt);
    assert!(
        store
            .read_deliveries(
                plan,
                &a,
                &authority(),
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 25
                }
            )
            .unwrap()
            .events
            .is_empty()
    );
}
