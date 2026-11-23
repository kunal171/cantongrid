//! Reproducible owned-PostgreSQL measurements; no HTTP, Ledger writes or production SLA claims.
use crate::{
    Consumer, DeliveryQuery, DeliveryStart, SubscriptionRequest, TemporalQuery,
    model_projection::tests::{PAYLOAD, Scenario},
};
use cantongrid_core::{
    lifecycle::Origin,
    visibility::{CallerGrants, PartyId},
};
use std::time::Instant;

/// Nearest-rank percentiles over measured complete calls, including SQL and JSON decoding.
fn timings(mut samples: Vec<u128>) -> serde_json::Value {
    samples.sort_unstable();
    let rank = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1].to_string();
    serde_json::json!({"unit":"microseconds","iterations":samples.len(),"p50":rank(50),"p95":rank(95),"max":samples.last().unwrap().to_string()})
}

#[test]
#[ignore = "manual owned PostgreSQL benchmark; synthetic authority, not a production capacity test"]
fn operations_benchmark() {
    let s = Scenario::visibility();
    let plan = &s.plans[0];
    let mut store = s.open();
    for part in 0..4 {
        let boundary = (part + 1) * 10;
        let position = serde_json::json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string();
        let mut batch = s.create(&position, &format!("contract-{:04}", part * 64), PAYLOAD);
        for n in 1..64 {
            batch.events.extend(
                s.create(
                    &position,
                    &format!("contract-{:04}", part * 64 + n),
                    PAYLOAD,
                )
                .events,
            );
        }
        for event in &mut batch.events {
            event.origin = Some(Origin {
                offset: boundary,
                snapshot_observation: part == 0,
            });
        }
        store.apply_authorized(plan, &batch, &s.lease()).unwrap();
    }
    let caller =
        CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let consumer = Consumer::from_verified("a".repeat(64), "b".repeat(64)).unwrap();
    let sub = store
        .subscribe(
            plan,
            &caller,
            &s.lease(),
            &consumer,
            SubscriptionRequest {
                name: "benchmark",
                start: DeliveryStart::Retained,
            },
        )
        .unwrap();
    let mut temporal = vec![];
    let mut delivery = vec![];
    // Five warmups precede fifty measured calls. Replaying one unacknowledged page
    // reuses its receipt, keeping consumer state and workload constant across samples.
    for n in 0..55 {
        let started = Instant::now();
        let page = store
            .read_as_of(
                plan,
                &caller,
                &s.lease(),
                TemporalQuery {
                    offset: 40,
                    after: None,
                    limit: 25,
                },
            )
            .unwrap();
        let elapsed = started.elapsed().as_micros();
        assert_eq!(page.visible_total, 256);
        assert_eq!(page.items.len(), 25);
        if n >= 5 {
            temporal.push(elapsed);
        }
        let started = Instant::now();
        let page = store
            .read_deliveries(
                plan,
                &caller,
                &s.lease(),
                &consumer,
                DeliveryQuery {
                    subscription: &sub.id,
                    after: None,
                    limit: 25,
                },
            )
            .unwrap();
        let elapsed = started.elapsed().as_micros();
        assert_eq!(page.events.len(), 25);
        if n >= 5 {
            delivery.push(elapsed);
        }
    }
    let report = serde_json::json!({"format_version":1,"scope":"owned_postgres_synthetic_authority","contracts":256,"batches":4,"caller_parties":1,"page_size":25,"connection":"reused_loopback","temporal":timings(temporal),"delivery_replay":timings(delivery),"http_included":false,"production_capacity_verified":false});
    println!("BENCHMARK {report}");
    // Scenario::drop deletes only this synthetic stream and its private subscription.
    drop(store);
}
