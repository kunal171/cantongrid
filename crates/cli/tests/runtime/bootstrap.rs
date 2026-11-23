//! Bootstrap uses real owned PostgreSQL and HTTP with mock source authority, without ledger writes.
use super::fixture::Fixture;
use super::*;

/// Inspect only our fixture-signed cursor; signatures are not encryption, so inspect public claims too.
fn claims(cursor: &str) -> Json {
    let mut validation = jsonwebtoken::Validation::new(Algorithm::HS256);
    validation.set_issuer(&["owned-tests"]);
    validation.set_audience(&["owned-api"]);
    jsonwebtoken::decode::<Json>(
        cursor,
        &jsonwebtoken::DecodingKey::from_secret(b"local-extended-cursor-test-key-00000001")
            .unwrap(),
        &validation,
    )
    .unwrap()
    .claims
}
/// Public bootstrap/snapshot metadata must not disclose source checkpoints or global ordinals.
fn private_progress_absent(value: &Json) {
    for field in [
        "baseline_offset",
        "baseline_after",
        "asOfOffset",
        "sourceLocalWatermark",
        "firstCheckpoint",
        "checkpoint",
        "delivery_id",
        "through",
    ] {
        assert!(!value.to_string().contains(field), "{field}");
    }
}
/// Extractor rejections use Axum's text errors; inspect status without assuming a product JSON body.
fn rejection_status(f: &Fixture, path: &str, body: Option<Json>) -> u16 {
    f.rt.block_on(async {
        let client = reqwest::Client::new();
        let request = if let Some(body) = body {
            client
                .post(format!("{}{path}", f.base))
                .header("Content-Type", "application/json")
                .body(body.to_string())
        } else {
            client.get(format!("{}{path}", f.base))
        };
        request
            .bearer_auth(f.token("a"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    })
}
/// Lifecycle updates between HTTP pages are replayed only after the immutable baseline is installed.
#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn bootstrap_api_pins_snapshot_during_updates_and_replays_the_matching_feed_after_restart() {
    let mut f = Fixture::new();
    f.commit(
        10,
        vec![
            f.create("a", "A", 2, true),
            f.create("z", "A", 3, true),
            f.create("b", "B", 4, true),
        ],
    );
    let registered = f.post(
        "/v1/subscriptions/bootstrap",
        "a",
        json!({"name":"baseline"}),
    );
    assert_eq!(registered.0, 200, "{}", registered.1);
    assert_eq!(registered.1["atomicHandoff"], true);
    private_progress_absent(&registered.1);
    private_progress_absent(&claims(registered.1["cursor"].as_str().unwrap()));
    let id = registered.1["id"].as_str().unwrap().to_owned();
    let snapshot = registered.1["snapshotUrl"].as_str().unwrap().to_owned();
    let feed = format!("/v1/subscriptions/{id}/events");
    let first = f.get(&format!("{snapshot}?limit=1"), "a");
    assert_eq!(first.0, 200, "{}", first.1);
    assert_eq!(first.1["visibleTotal"], "2");
    assert_eq!(first.1["items"][0]["contractId"], "a");
    assert!(
        first
            .1
            .to_string()
            .contains("12345678901234567890.0000000001")
    );
    private_progress_absent(&first.1);
    let cursor = first.1["nextCursor"].as_str().unwrap();
    let decoded = claims(cursor);
    assert_eq!(decoded["kind"], format!("bootstrap:{id}"));
    assert_eq!(decoded["after"], "a");
    private_progress_absent(&decoded);
    assert_eq!(f.get(&format!("{feed}?cursor={cursor}"), "a").0, 400);
    assert_eq!(
        f.get(
            &format!(
                "{snapshot}?cursor={}",
                registered.1["cursor"].as_str().unwrap()
            ),
            "a"
        )
        .0,
        400
    );
    assert_eq!(
        f.get(&snapshot, "b"),
        f.get(
            "/v1/subscriptions/00000000-0000-0000-0000-000000000000/snapshot",
            "b"
        )
    );
    let before = f.get(&feed, "a").1;
    f.commit(20, vec![f.create("hidden", "B", 15, false)]);
    assert_eq!(f.get(&feed, "a").1, before);
    assert_eq!(
        f.get(&format!("{snapshot}?limit=1"), "a").1["items"],
        first.1["items"]
    );
    let mut archived = f.create("z", "A", 25, false);
    archived.id = "archive-z".into();
    archived.witnesses.clear();
    archived.action = Action::ArchiveByPriorContract;
    f.commit(30, vec![archived, f.create("new", "A", 25, false)]);
    let retry = f.post(
        "/v1/subscriptions/bootstrap",
        "a",
        json!({"name":"baseline"}),
    );
    assert_eq!(retry.0, 200);
    assert_eq!(retry.1["id"], id);
    f.restart();
    let last = f.get(&format!("{snapshot}?limit=1&cursor={cursor}"), "a");
    assert_eq!(last.0, 200, "{}", last.1);
    assert_eq!(last.1["items"][0]["contractId"], "z");
    assert_eq!(last.1["visibleTotal"], "2");
    assert!(last.1["nextCursor"].is_null());
    let events = f.get(&feed, "a");
    assert_eq!(events.0, 200);
    assert_eq!(events.1["events"].as_array().unwrap().len(), 2);
    assert_eq!(events.1["events"][0]["eventId"], "archive-z");
    assert_eq!(events.1["events"][1]["eventId"], "create-new");
    assert_eq!(
        f.post(
            &format!("/v1/subscriptions/{id}/acknowledge"),
            "a",
            json!({"cursor":events.1["nextCursor"]})
        )
        .0,
        200
    );
    assert!(f.get(&feed, "a").1["events"].as_array().unwrap().is_empty());
    assert_eq!(f.get(&snapshot, "a").0, 409);
    assert_eq!(
        f.post(
            "/v1/subscriptions/bootstrap",
            "a",
            json!({"name":"baseline"})
        )
        .0,
        409
    );
    // New baseline means a new explicit name; no old snapshot is silently moved to the latest head.
    let fresh = f.post("/v1/subscriptions/bootstrap", "a", json!({"name":"fresh"}));
    assert_eq!(fresh.0, 200);
    let fresh_path = fresh.1["snapshotUrl"].as_str().unwrap();
    assert_eq!(f.get(&format!("{fresh_path}?cursor={cursor}"), "a").0, 400);
    let current = f.get(fresh_path, "a");
    assert_eq!(current.1["items"][1]["contractId"], "new");
}
#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn bootstrap_api_rechecks_authority_and_reports_retention_and_coverage_failures_explicitly() {
    let mut f = Fixture::new();
    f.commit(10, vec![f.create("a", "A", 2, true)]);
    let registered = f.post(
        "/v1/subscriptions/bootstrap",
        "a",
        json!({"name":"baseline"}),
    );
    assert_eq!(registered.0, 200);
    let snapshot = registered.1["snapshotUrl"].as_str().unwrap();
    assert_eq!(
        rejection_status(&f, &format!("{snapshot}?asOfOffset=10"), None),
        400
    );
    assert_eq!(f.get(&format!("{snapshot}?limit=26"), "a").0, 400);
    assert_eq!(
        rejection_status(
            &f,
            "/v1/subscriptions/bootstrap",
            Some(json!({"name":"bad","parties":["B"]}))
        ),
        422
    );
    assert_eq!(
        f.post(
            "/v1/subscriptions/bootstrap",
            "unmapped",
            json!({"name":"bad"})
        )
        .0,
        401
    );
    f.healthy.store(false, Ordering::SeqCst);
    assert_eq!(f.get(snapshot, "a").0, 503);
    f.healthy.store(true, Ordering::SeqCst);
    f.allowed.store(false, Ordering::SeqCst);
    assert_eq!(f.get(snapshot, "a").0, 503);
    f.allowed.store(true, Ordering::SeqCst);
    f.commit(
        20,
        vec![
            f.create("later", "A", 15, false),
            f.create("latest", "A", 16, false),
        ],
    );
    f.store
        .prune_deliveries(&f.plan, &f.lease(), 1, 256)
        .unwrap();
    let gap = f.get(snapshot, "a");
    assert_eq!(gap.0, 409);
    assert_eq!(gap.1["coverage"], "gap");
    assert!(gap.1.get("items").is_none());
    let retry = f.post(
        "/v1/subscriptions/bootstrap",
        "a",
        json!({"name":"baseline"}),
    );
    assert_eq!(retry.0, 409);
    assert_eq!(retry.1["coverage"], "gap");
    let fresh = f.post("/v1/subscriptions/bootstrap", "a", json!({"name":"fresh"}));
    assert_eq!(fresh.0, 200);
    let fresh_path = fresh.1["snapshotUrl"].as_str().unwrap();
    f.revoke_a();
    assert_eq!(f.get(fresh_path, "a").0, 401);
}
