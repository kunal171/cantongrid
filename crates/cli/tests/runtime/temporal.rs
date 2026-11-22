//! REST/GraphQL parity and current-authority checks for exact canonical source-boundary reads.
use super::fixture::Fixture;
use super::*;

#[test]
#[ignore = "requires owned PostgreSQL and mock source; no Canton writes"]
fn temporal_api_proves_boundaries_and_rejects_cross_offset_cursors_or_revoked_grants() {
    let mut f = Fixture::new();
    f.commit(
        10,
        vec![
            f.create("a", "A", 3, true),
            f.create("a2", "A", 4, true),
            f.create("b", "B", 5, true),
        ],
    );
    let mut archived = f.create("a", "A", 15, false);
    archived.id = "archive-a".into();
    archived.witnesses.clear();
    archived.action = Action::ArchiveByPriorContract;
    f.commit(20, vec![archived, f.create("new", "A", 15, false)]);
    let path = "/v1/contracts/as-of?asOfOffset=10&limit=1";
    let first = f.get(path, "a");
    assert_eq!(first.0, 200);
    assert_eq!(first.1["visibleTotal"], "2");
    assert_eq!(first.1["items"][0]["contractId"], "a");
    assert_eq!(first.1["provenance"]["coverage"], "observed_retained");
    assert_eq!(first.1["provenance"]["asOfOffset"], "10");
    assert!(!first.1.to_string().contains("witnesses"));
    assert!(
        first
            .1
            .to_string()
            .contains("12345678901234567890.0000000001")
    );
    let cursor = first.1["nextCursor"].as_str().unwrap();
    let operation = json!({"query":"query($cursor:String){contractsAsOf(asOfOffset:\"10\",limit:1,cursor:$cursor){items visibleTotal nextCursor provenance}}","variables":{"cursor":cursor}});
    let graphql = f.post("/graphql", "a", operation);
    assert_eq!(graphql.0, 200);
    assert!(graphql.1.get("errors").is_none(), "{}", graphql.1);
    let next = f.get(&format!("{path}&cursor={cursor}"), "a");
    assert_eq!(graphql.1["data"]["contractsAsOf"], next.1);
    assert_eq!(
        f.get(
            &format!("/v1/contracts/as-of?asOfOffset=15&cursor={cursor}"),
            "a"
        )
        .0,
        400
    );
    assert_eq!(f.get(&format!("{path}&cursor={cursor}"), "b").0, 400);
    assert_eq!(f.get(&format!("/v1/contracts?cursor={cursor}"), "a").0, 400);
    let at15 = f.get("/v1/contracts/as-of?asOfOffset=15", "a").1;
    assert_eq!(at15["visibleTotal"], "2");
    assert!(
        !at15["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["contractId"] == "a")
    );
    assert_eq!(
        f.get("/v1/contracts/as-of?asOfOffset=10", "b").1["visibleTotal"],
        "1"
    );
    for boundary in ["9", "21"] {
        let unavailable = f.get(&format!("/v1/contracts/as-of?asOfOffset={boundary}"), "a");
        assert_eq!(unavailable.0, 409);
        assert_eq!(unavailable.1["coverage"], "unavailable");
        assert!(unavailable.1.get("visibleTotal").is_none());
    }
    let unavailable = f.post(
        "/graphql",
        "a",
        json!({"query":"{contractsAsOf(asOfOffset:\"9\"){visibleTotal}}"}),
    );
    assert_eq!(
        unavailable.1["errors"][0]["extensions"]["coverage"],
        "unavailable"
    );
    for boundary in ["01", "1.0", "9223372036854775808"] {
        assert_eq!(
            f.get(&format!("/v1/contracts/as-of?asOfOffset={boundary}"), "a")
                .0,
            400
        );
    }
    f.healthy.store(false, Ordering::Relaxed);
    assert_eq!(f.get(path, "a").0, 503);
    f.healthy.store(true, Ordering::Relaxed);
    f.allowed.store(false, Ordering::Relaxed);
    assert_eq!(f.get(path, "a").0, 503);
    f.allowed.store(true, Ordering::Relaxed);
    f.revoke_a();
    assert_eq!(f.get(path, "a").0, 401);
    assert_eq!(f.get(&format!("{path}&cursor={cursor}"), "ab").0, 400);
}
