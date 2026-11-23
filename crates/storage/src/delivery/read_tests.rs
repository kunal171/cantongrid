//! Replay, monotonic ACK and bounded backpressure controls.
use super::*;

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn durable_delivery_replays_unacked_pages_and_monotonic_acks_survive_a_new_connection() {
    let s = Scenario::visibility();
    s.seed();
    let plan = &s.plans[0];
    let lease = s.lease();
    let mut store = s.open();
    let caller =
        CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let c = consumer();
    let sub = store
        .subscribe(
            plan,
            &caller,
            &lease,
            &c,
            SubscriptionRequest {
                name: "consumer",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let query = |store: &mut crate::Store, after| {
        store.read_deliveries(
            plan,
            &caller,
            &lease,
            &c,
            DeliveryQuery {
                subscription: &sub.id,
                after,
                limit: 1,
            },
        )
    };
    let first = query(&mut store, None).unwrap();
    let replay = query(&mut s.open(), None).unwrap();
    assert_eq!(first.events, replay.events);
    assert_eq!(first.receipt, replay.receipt);
    assert_eq!(first.events[0]["eventId"], "create-first");
    let mut next = s.create("second", "second", PAYLOAD);
    next.events[0].id = "create-second".into();
    store.apply_authorized(plan, &next, &lease).unwrap();
    let second = query(&mut store, first.receipt.as_deref()).unwrap();
    assert_eq!(second.events[0]["eventId"], "create-second");
    store
        .acknowledge(
            plan,
            &caller,
            &lease,
            &c,
            &sub.id,
            second.receipt.as_deref().unwrap(),
        )
        .unwrap();
    store
        .acknowledge(
            plan,
            &caller,
            &lease,
            &c,
            &sub.id,
            first.receipt.as_deref().unwrap(),
        )
        .unwrap();
    assert!(query(&mut s.open(), None).unwrap().events.is_empty());
    let current = store.checkpoint(plan).unwrap();
    let mut invalid = s.lease();
    invalid.invalidate();
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &caller,
                &invalid,
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 1
                }
            )
            .err(),
        Some(StoreError::Authorization)
    );
    assert_eq!(store.checkpoint(plan).unwrap(), current);
}

#[test]
#[ignore = "requires owned PostgreSQL; bounded receipt budget"]
fn receipt_backpressure_requires_ack_and_repeated_pages_do_not_allocate_more_positions() {
    let s = Scenario::visibility();
    s.seed();
    let plan = &s.plans[0];
    let authority = s.lease();
    let mut store = s.open();
    let caller =
        CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let c = consumer();
    let mut batch = s.create("many", "item0", PAYLOAD);
    for n in 1..64 {
        batch
            .events
            .extend(s.create("many", &format!("item{n}"), PAYLOAD).events);
    }
    store.apply_authorized(plan, &batch, &authority).unwrap();
    let sub = store
        .subscribe(
            plan,
            &caller,
            &authority,
            &c,
            SubscriptionRequest {
                name: "backpressure",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let mut after = None;
    let mut first = None;
    for n in 0..64 {
        let page = store
            .read_deliveries(
                plan,
                &caller,
                &authority,
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: after.as_deref(),
                    limit: 1,
                },
            )
            .unwrap();
        if n == 0 {
            first = page.receipt.clone();
        }
        after = page.receipt;
    }
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &caller,
                &authority,
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: after.as_deref(),
                    limit: 1
                }
            )
            .err(),
        Some(StoreError::DeliveryBackpressure)
    );
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &caller,
                &authority,
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 1
                }
            )
            .unwrap()
            .receipt,
        first
    );
    store
        .acknowledge(
            plan,
            &caller,
            &authority,
            &c,
            &sub.id,
            after.as_deref().unwrap(),
        )
        .unwrap();
    store
        .acknowledge(
            plan,
            &caller,
            &authority,
            &c,
            &sub.id,
            first.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(
        store
            .read_deliveries(
                plan,
                &caller,
                &authority,
                &c,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 1
                }
            )
            .unwrap()
            .events
            .len(),
        1
    );
}
