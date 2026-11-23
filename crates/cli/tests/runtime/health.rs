//! Public readiness is boolean-only; operator counters remain behind owned DB/full-source authority.
use super::fixture::Fixture;
use super::*;

#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn readiness_reauthenticates_and_never_discloses_full_source_metrics() {
    let mut f = Fixture::new();
    let url = format!("{}/v1/health", f.base);
    assert_eq!(request(&f.rt, &url, None, None).0, 401);
    assert_eq!(f.get("/v1/health", "a").0, 503);
    f.commit(
        10,
        vec![
            f.create("private-a", "A", 2, true),
            f.create("private-b", "B", 3, true),
        ],
    );
    let ready = f.get("/v1/health", "a");
    assert_eq!(ready.0, 200);
    assert_eq!(ready.1, json!({"status":"ready","formatVersion":1}));
    let checkpoint = f.store.checkpoint(&f.plan).unwrap();
    let lease = f.lease();
    let before = f.store.operational_metrics(&f.plan, &lease).unwrap();
    assert_eq!(before["activeSubscriptions"], "0");
    let subscription = f.post(
        "/v1/subscriptions",
        "a",
        json!({"name":"metrics-test","start":"retained"}),
    );
    let after = f.store.operational_metrics(&f.plan, &lease).unwrap();
    assert_eq!(after["activeSubscriptions"], "1");
    assert_eq!(ready, f.get("/v1/health", "b"));
    assert_eq!(checkpoint, f.store.checkpoint(&f.plan).unwrap());
    let now = Instant::now();
    let partial = SourceLease::from_verified(
        f.plan.context().binding(),
        [party("A")],
        now,
        MAX_SOURCE_LEASE,
        now + MAX_SOURCE_LEASE,
    )
    .unwrap();
    assert_eq!(
        f.store.operational_metrics(&f.plan, &partial).err(),
        Some(cantongrid_storage::StoreError::Authorization)
    );
    let (source, _, _) = f.plan.context().identity();
    let mut destination = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    destination.execute("UPDATE cantongrid.streams SET observed_at=now()-interval '1 minute' WHERE definition->>'source'=$1",&[&source]).unwrap();
    assert_eq!(f.get("/v1/health", "a").0, 503);
    let id = subscription.1["id"].as_str().unwrap();
    assert_eq!(f.get(&format!("/v1/subscriptions/{id}/events"), "a").0, 503);
    assert_eq!(f.get("/v1/contracts/as-of?asOfOffset=10", "a").0, 200);
    assert_eq!(
        f.store.operational_metrics(&f.plan, &lease).unwrap()["ingestionFresh"],
        false
    );
    destination
        .execute(
            "UPDATE cantongrid.streams SET observed_at=now() WHERE definition->>'source'=$1",
            &[&source],
        )
        .unwrap();
    f.healthy.store(false, Ordering::Relaxed);
    assert_eq!(f.get("/v1/health", "a").0, 503);
    f.healthy.store(true, Ordering::Relaxed);
    f.allowed.store(false, Ordering::Relaxed);
    assert_eq!(f.get("/v1/health", "a").0, 503);
    f.allowed.store(true, Ordering::Relaxed);
    f.revoke_a();
    assert_eq!(f.get("/v1/health", "a").0, 401);
}
