//! Retention distinguishes authorized loss from hidden activity and preserves canonical history.
use super::*;

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn retention_records_only_visible_unacked_loss_and_preserves_temporal_state() {
    let mut s = Scenario::visibility();
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [s.models[0].template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    let plan = &s.plans[0];
    let authority = crate::model_projection::tests::lease(plan, vec!["A", "B"]);
    let mut store = s.open();
    let boundary =
        serde_json::json!({"snapshot":false,"boundary":10,"through":10,"offset":10,"pk":i64::MAX})
            .to_string();
    let mut batch = s.create(&boundary, "a", PAYLOAD);
    batch
        .events
        .extend(s.create(&boundary, "b", PAYLOAD).events);
    batch
        .events
        .extend(s.create(&boundary, "b2", PAYLOAD).events);
    for (index, event) in batch.events.iter_mut().enumerate() {
        event.origin = Some(cantongrid_core::lifecycle::Origin {
            offset: index as i64 + 1,
            snapshot_observation: true,
        });
        if index > 0 {
            event.witnesses = [PartyId::new("B").unwrap()].into();
        }
    }
    store.apply_authorized(plan, &batch, &authority).unwrap();
    let a = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let b = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("B").unwrap()]);
    let ca = consumer();
    let cb = Consumer::from_verified("c".repeat(64), "d".repeat(64)).unwrap();
    let sa = store
        .subscribe(
            plan,
            &a,
            &authority,
            &ca,
            SubscriptionRequest {
                name: "a",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let sb = store
        .subscribe(
            plan,
            &b,
            &authority,
            &cb,
            SubscriptionRequest {
                name: "b",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let page = store
        .read_deliveries(
            plan,
            &a,
            &authority,
            &ca,
            DeliveryQuery {
                subscription: &sa.id,
                after: None,
                limit: 25,
            },
        )
        .unwrap();
    store
        .acknowledge(
            plan,
            &a,
            &authority,
            &ca,
            &sa.id,
            page.receipt.as_deref().unwrap(),
        )
        .unwrap();
    let b_received = store
        .read_deliveries(
            plan,
            &b,
            &authority,
            &cb,
            DeliveryQuery {
                subscription: &sb.id,
                after: None,
                limit: 1,
            },
        )
        .unwrap();
    let before = store
        .read_as_of(
            plan,
            &a,
            &authority,
            crate::TemporalQuery {
                offset: 10,
                after: None,
                limit: 25,
            },
        )
        .unwrap();
    let checkpoint = store.checkpoint(plan).unwrap();
    assert_eq!(store.prune_deliveries(plan, &authority, 1, 256).unwrap(), 2);
    assert!(
        store
            .read_deliveries(
                plan,
                &a,
                &authority,
                &ca,
                DeliveryQuery {
                    subscription: &sa.id,
                    after: None,
                    limit: 25
                }
            )
            .unwrap()
            .events
            .is_empty()
    );
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &b,
                &authority,
                &cb,
                DeliveryQuery {
                    subscription: &sb.id,
                    after: None,
                    limit: 25
                }
            )
            .err(),
        Some(StoreError::DeliveryGap)
    );
    // An already issued private receipt survives outbox pruning. A late ACK can safely
    // cover that processed prefix; a missing receipt would require explicit resynchronization.
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &b,
                &authority,
                &cb,
                DeliveryQuery {
                    subscription: &sb.id,
                    after: b_received.receipt.as_deref(),
                    limit: 25
                }
            )
            .unwrap()
            .events[0]["contractId"],
        "b2"
    );
    store
        .acknowledge(
            plan,
            &b,
            &authority,
            &cb,
            &sb.id,
            b_received.receipt.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &b,
                &authority,
                &cb,
                DeliveryQuery {
                    subscription: &sb.id,
                    after: None,
                    limit: 25
                }
            )
            .unwrap()
            .events
            .len(),
        1
    );
    let after = store
        .read_as_of(
            plan,
            &a,
            &authority,
            crate::TemporalQuery {
                offset: 10,
                after: None,
                limit: 25,
            },
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(store.checkpoint(plan).unwrap(), checkpoint);
}
