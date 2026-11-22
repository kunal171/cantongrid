//! Durable progress is private and caller-bound; these fixtures use synthetic authority only.
use super::*;
use crate::{
    StoreError,
    model_projection::tests::{PAYLOAD, Scenario},
};
use cantongrid_core::visibility::{CallerGrants, PartyId};

/// Stable digests stand in for an already authenticated organization principal and grant binding.
fn consumer() -> Consumer {
    Consumer::from_verified("a".repeat(64), "b".repeat(64)).unwrap()
}
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn subscriptions_retry_without_resetting_and_reject_other_principals_parties_and_expiry() {
    let s = Scenario::visibility();
    s.seed();
    let plan = &s.plans[0];
    let mut store = s.open();
    let lease = s.lease();
    let a = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let c = consumer();
    let first = store
        .subscribe(
            plan,
            &a,
            &lease,
            &c,
            SubscriptionRequest {
                name: "dashboard",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let again = store
        .subscribe(
            plan,
            &a,
            &lease,
            &c,
            SubscriptionRequest {
                name: "dashboard",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(first.receipt, again.receipt);
    assert_eq!(
        store
            .subscribe(
                plan,
                &a,
                &lease,
                &c,
                SubscriptionRequest {
                    name: "dashboard",
                    start: DeliveryStart::Now
                }
            )
            .err(),
        Some(StoreError::ModelQuery)
    );
    let stranger = Consumer::from_verified("c".repeat(64), "d".repeat(64)).unwrap();
    assert_eq!(
        store.acknowledge(plan, &a, &lease, &stranger, &first.id, &first.receipt),
        Err(StoreError::SubscriptionUnavailable)
    );
    assert_eq!(
        store.acknowledge(plan, &a, &lease, &c, &first.id, "not-a-receipt"),
        Err(StoreError::SubscriptionUnavailable)
    );
    store
        .acknowledge(plan, &a, &lease, &c, &first.id, &first.receipt)
        .unwrap();
    store
        .acknowledge(plan, &a, &lease, &c, &first.id, &first.receipt)
        .unwrap();
    store.client.execute("UPDATE cantongrid.delivery_subscriptions SET expires_at=now()-interval '1 second' WHERE id=$1",&[&first.id]).unwrap();
    assert_eq!(
        store.acknowledge(plan, &a, &lease, &c, &first.id, &first.receipt),
        Err(StoreError::SubscriptionUnavailable)
    );
    let fresh = store
        .subscribe(
            plan,
            &a,
            &lease,
            &c,
            SubscriptionRequest {
                name: "dashboard",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    assert_ne!(fresh.id, first.id);
    store
        .cancel_subscription(plan, &a, &lease, &c, &fresh.id)
        .unwrap();
    assert_eq!(
        store.cancel_subscription(plan, &a, &lease, &c, &fresh.id),
        Err(StoreError::SubscriptionUnavailable)
    );
    assert!(Consumer::from_verified("raw-private-subject".into(), "b".repeat(64)).is_err());
    assert!(
        store
            .subscribe(
                plan,
                &a,
                &lease,
                &c,
                SubscriptionRequest {
                    name: "../bad",
                    start: DeliveryStart::Now
                }
            )
            .is_err()
    );
    // Subscription state changes do not modify canonical exact values or checkpoints.
    assert!(store.client.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.contracts WHERE stream_key=$1 AND payload->>'value'=$2)",&[&stream_key(plan).0,&PAYLOAD]).unwrap().get::<_,bool>(0));
}

#[path = "read_tests.rs"]
mod read_controls;

#[path = "retention_tests.rs"]
mod retention_controls;
