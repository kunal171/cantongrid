//! Durable REST delivery tests use real owned storage and the common synthetic source authority.
use super::fixture::Fixture;
use super::*;

/// Decode our own fixture-signed cursor to prove it contains no private delivery sequence.
fn cursor_receipt(token: &str) -> String {
    let mut validation = jsonwebtoken::Validation::new(Algorithm::HS256);
    validation.set_issuer(&["owned-tests"]);
    validation.set_audience(&["owned-api"]);
    let data = jsonwebtoken::decode::<Json>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(b"local-extended-cursor-test-key-00000001")
            .unwrap(),
        &validation,
    )
    .unwrap()
    .claims;
    let receipt = data["after"].as_str().unwrap();
    assert_eq!(receipt.len(), 36);
    assert!(receipt.contains('-'));
    receipt.into()
}
#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn durable_api_replay_ack_scope_privacy_and_authorized_retention_gaps() {
    let mut f = Fixture::new();
    f.commit(
        10,
        vec![f.create("a", "A", 2, true), f.create("b", "B", 3, true)],
    );
    let a = f.post(
        "/v1/subscriptions",
        "a",
        json!({"name":"dashboard","start":"retained"}),
    );
    assert_eq!(a.0, 200, "{}", a.1);
    let again = f.post(
        "/v1/subscriptions",
        "a",
        json!({"name":"dashboard","start":"retained"}),
    );
    assert_eq!(a.1["id"], again.1["id"]);
    let id = a.1["id"].as_str().unwrap();
    let initial = a.1["cursor"].as_str().unwrap();
    cursor_receipt(initial);
    let b = f.post(
        "/v1/subscriptions",
        "b",
        json!({"name":"dashboard","start":"retained"}),
    );
    let bid = b.1["id"].as_str().unwrap();
    let path = format!("/v1/subscriptions/{id}/events?limit=1");
    let page = f.get(&format!("{path}&cursor={initial}"), "a");
    assert_eq!(page.0, 200, "{}", page.1);
    assert_eq!(page.1["formatVersion"], 1);
    assert_eq!(page.1["events"][0]["eventId"], "create-a");
    assert_eq!(page.1["more"], false);
    assert_eq!(page.1["events"], f.get(&path, "a").1["events"]);
    for secret in [
        "witnesses",
        "envelope",
        "delivery_id",
        "sourceLocalWatermark",
        "position",
        "authorizationBinding",
    ] {
        assert!(!page.1.to_string().contains(secret), "{secret}");
    }
    assert!(
        page.1["events"][0]["payload"]["value"]
            .as_str()
            .unwrap()
            .contains("12345678901234567890.0000000001")
    );
    let cursor = page.1["nextCursor"].as_str().unwrap();
    cursor_receipt(cursor);
    assert_eq!(
        f.get(&path, "b"),
        f.get(
            "/v1/subscriptions/00000000-0000-0000-0000-000000000000/events",
            "b"
        )
    );
    assert_eq!(
        f.get(
            &format!("/v1/subscriptions/{bid}/events?cursor={cursor}"),
            "a"
        )
        .0,
        400
    );
    assert_eq!(f.get(&format!("/v1/contracts?cursor={cursor}"), "a").0, 400);
    assert_eq!(
        f.post(
            &format!("/v1/subscriptions/{id}/acknowledge"),
            "a",
            json!({"cursor":cursor})
        )
        .0,
        200
    );
    f.commit(20, vec![f.create("later", "A", 15, false)]);
    let later = f.get(&path, "a");
    assert_eq!(later.1["events"][0]["eventId"], "create-later");
    let next = later.1["nextCursor"].as_str().unwrap();
    assert_eq!(
        f.post(
            &format!("/v1/subscriptions/{id}/acknowledge"),
            "a",
            json!({"cursor":next})
        )
        .0,
        200
    );
    assert_eq!(
        f.post(
            &format!("/v1/subscriptions/{id}/acknowledge"),
            "a",
            json!({"cursor":cursor})
        )
        .0,
        200
    );
    let empty = f.get(&format!("{path}&cursor={next}"), "a");
    f.commit(30, vec![f.create("hidden-later", "B", 25, false)]);
    assert_eq!(empty, f.get(&format!("{path}&cursor={next}"), "a"));
    let lease = f.lease();
    f.store.prune_deliveries(&f.plan, &lease, 1, 256).unwrap();
    assert_eq!(f.get(&path, "a").0, 200);
    let gap = f.get(&format!("/v1/subscriptions/{bid}/events"), "b");
    assert_eq!(gap.0, 409);
    assert_eq!(gap.1["coverage"], "gap");
    assert!(gap.1.get("events").is_none());
    f.revoke_a();
    assert_eq!(f.get(&path, "a").0, 401);
}
#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn long_poll_reauthorizes_after_source_revocation_instead_of_returning_a_heartbeat() {
    let mut f = Fixture::new();
    f.commit(10, vec![]);
    let sub = f.post(
        "/v1/subscriptions",
        "a",
        json!({"name":"poller","start":"now"}),
    );
    assert_eq!(sub.0, 200);
    let id = sub.1["id"].as_str().unwrap();
    let allowed = f.allowed.clone();
    let changed = thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        allowed.store(false, Ordering::Relaxed);
    });
    let start = Instant::now();
    let result = f.get(&format!("/v1/subscriptions/{id}/events?waitSeconds=2"), "a");
    changed.join().unwrap();
    assert_eq!(result.0, 503);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(result.1.get("events").is_none());
    f.allowed.store(true, Ordering::Relaxed);
    assert_eq!(
        f.get(
            &format!("/v1/subscriptions/{id}/events?waitSeconds=11"),
            "a"
        )
        .0,
        400
    );
    assert_eq!(
        f.post(&format!("/v1/subscriptions/{id}/cancel"), "a", json!({}))
            .0,
        200
    );
    assert_eq!(f.get(&format!("/v1/subscriptions/{id}/events"), "a").0, 404);
}
