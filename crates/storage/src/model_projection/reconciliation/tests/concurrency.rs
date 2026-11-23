//! Read snapshots pin both lifecycle state and active model revision while a separate writer commits.
use super::*;
use std::time::Duration;

/// Archive/create and model activation on another connection must not tear an in-flight report.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn concurrent_ingestion_and_promotion_preserve_one_coherent_snapshot() {
    let mut s = scenario();
    seed(&s);
    let old = bind(&s);
    let mut next = s.models[0].clone();
    next.revision = 2;
    s.install(next);
    let result = s
        .open()
        .read_reconciliation_checked(
            &s.plans[0],
            &caller(&s, &["A", "B"]),
            &authority(&s),
            &old,
            &policy(),
            || {
                let mut later = batch(
                    &s,
                    20,
                    "private-contract-violated",
                    &payload("0", "0", "Ready"),
                    false,
                    "B",
                );
                later.events[0].id = "archive-violated".into();
                later.events[0].witnesses.clear();
                later.events[0].action = Action::ArchiveByPriorContract;
                later.events.extend(
                    batch(
                        &s,
                        20,
                        "private-new-balanced",
                        &payload("0.1", "0.2", "Ready"),
                        false,
                        "B",
                    )
                    .events,
                );
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| {
                            s.open()
                                .apply_authorized(&s.plans[0], &later, &authority(&s))
                                .unwrap();
                            s.open()
                                .bind_model(
                                    &s.plans[0],
                                    &authority(&s),
                                    &s.namespace,
                                    &s.models[0].name,
                                    2,
                                    Some(1),
                                )
                                .unwrap();
                        })
                        .join()
                        .unwrap()
                });
                Ok(())
            },
        )
        .unwrap();
    let before = serde_json::to_value(result).unwrap();
    assert_eq!(before["model"]["revision"], "1");
    assert_eq!(before["contracts"], "2");
    assert_eq!(before["comparison"], "violated");
    assert_eq!(
        read(&s, &old).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    let current = s
        .open()
        .bound_models(&s.plans[0], &caller(&s, &["A", "B"]), &authority(&s))
        .unwrap()
        .remove(0);
    let after = read(&s, &current).unwrap();
    assert_eq!(after["model"]["revision"], "2");
    assert_eq!(after["contracts"], "2");
    assert_eq!(after["comparison"], "satisfied");
    assert_ne!(
        before["definitionFingerprint"],
        after["definitionFingerprint"]
    );
}

/// Lease expiry after snapshot acquisition must discard even an otherwise complete report.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn source_evidence_expiring_during_the_read_discards_the_result() {
    let s = scenario();
    seed(&s);
    let binding = bind(&s);
    let now = Instant::now();
    let authority = SourceLease::from_verified(
        s.plans[0].context().binding(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()],
        now,
        Duration::from_millis(100),
        now + Duration::from_secs(1),
    )
    .unwrap();
    let result = s.open().read_reconciliation_checked(
        &s.plans[0],
        &caller(&s, &["A", "B"]),
        &authority,
        &binding,
        &policy(),
        || {
            std::thread::sleep(Duration::from_millis(150));
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err(), StoreError::Authorization);
}
