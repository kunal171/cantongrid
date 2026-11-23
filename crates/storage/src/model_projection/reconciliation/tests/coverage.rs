//! Missing, unfinished and stale source evidence cannot be converted into a successful zero.
use super::*;

/// Replace only this scenario's cursor metadata, allowing unsupported cursor shapes to be tested safely.
fn set_cursor(s: &Scenario, checkpoint: &str) {
    let key = stream_key(&s.plans[0]).0;
    let mut store = s.open();
    let mut tx = store.client.transaction().unwrap();
    tx.execute(
        "UPDATE cantongrid.streams SET checkpoint=$2 WHERE stream_key=$1",
        &[&key, &checkpoint],
    )
    .unwrap();
    tx.execute(
        "UPDATE cantongrid.batches SET position=$2 WHERE stream_key=$1 AND revision=1",
        &[&key, &checkpoint],
    )
    .unwrap();
    tx.commit().unwrap();
}
/// A valid schema is insufficient when the snapshot handoff, journal or worker observation is incomplete.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn unfinished_pruned_unknown_and_stale_evidence_never_produces_a_report() {
    let s = scenario();
    seed(&s);
    let binding = bind(&s);
    let key = stream_key(&s.plans[0]).0;
    for cursor in [
        json!({"snapshot":true,"boundary":10,"through":10,"offset":10,"pk":1}).to_string(),
        json!({"snapshot":false,"boundary":10,"through":20,"offset":15,"pk":1}).to_string(),
        json!({"snapshot":false,"boundary":10,"through":20,"offset":20,"pk":i64::MAX}).to_string(),
        position(10).replace('}', ",\"future\":true}"),
        position(10).replace('}', ",\"boundary\":10}"),
    ] {
        set_cursor(&s, &cursor);
        assert_eq!(
            read(&s, &binding).unwrap_err(),
            StoreError::ReconciliationUnavailable
        );
    }
    set_cursor(&s, &position(10));
    let mut store = s.open();
    store.client.execute("UPDATE cantongrid.streams SET observed_at=now()-interval '31 seconds' WHERE stream_key=$1",&[&key]).unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    store
        .observe_authorized(&s.plans[0], &authority(&s))
        .unwrap();
    assert!(read(&s, &binding).is_ok());
    // An additive selection floor newer than the head cannot establish complete current coverage.
    let manifest = json!({"checkpoint":position(20)});
    store.client.execute("INSERT INTO cantongrid.selection_upgrades(stream_key,fingerprint,manifest) VALUES($1,'future-floor',$2)",&[&key,&manifest]).unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    store
        .client
        .execute(
            "DELETE FROM cantongrid.selection_upgrades WHERE stream_key=$1",
            &[&key],
        )
        .unwrap();
    store
        .client
        .execute(
            "DELETE FROM cantongrid.batches WHERE stream_key=$1 AND revision=1",
            &[&key],
        )
        .unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
}

/// Both absence and a nonempty but entirely filtered template set are unavailable, not vacuous success.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn empty_and_all_filtered_selections_are_explicitly_unavailable() {
    for filtered in [false, true] {
        let s = scenario();
        let mut initial = batch(&s, 10, "excluded", &payload("0", "0", "Closed"), true, "A");
        if !filtered {
            initial.events.clear();
        }
        s.open()
            .apply_authorized(&s.plans[0], &initial, &authority(&s))
            .unwrap();
        let binding = bind(&s);
        assert_eq!(
            read(&s, &binding).unwrap_err(),
            StoreError::ReconciliationUnavailable
        );
    }
}

/// Reject excess canonical rows before filtering and oversized payloads before transferring them to Rust.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn canonical_row_and_byte_limits_fail_without_truncating_or_ignoring_filtered_inputs() {
    let s = scenario();
    let mut initial = batch(
        &s,
        10,
        "private-0",
        &payload("0.1", "0.2", "Ready"),
        true,
        "A",
    );
    for n in 1..128 {
        initial.events.extend(
            batch(
                &s,
                10,
                &format!("private-{n}"),
                &payload("0.1", "0.2", "Ready"),
                true,
                "B",
            )
            .events,
        );
    }
    // Match the real reader's 64-event pages, then acknowledge the completed snapshot handoff.
    for (index, events) in initial.events.chunks(64).enumerate() {
        let page = Batch {
            context: s.plans[0].context().clone(),
            position: json!({"snapshot":true,"boundary":10,"through":10,"offset":10,"pk":index+1})
                .to_string(),
            events: events.to_vec(),
        };
        s.open()
            .apply_authorized(&s.plans[0], &page, &authority(&s))
            .unwrap();
    }
    s.open()
        .apply_authorized(
            &s.plans[0],
            &Batch {
                context: s.plans[0].context().clone(),
                position: position(10),
                events: vec![],
            },
            &authority(&s),
        )
        .unwrap();
    let binding = bind(&s);
    assert_eq!(read(&s, &binding).unwrap()["contracts"], "128");
    let filtered = batch(
        &s,
        20,
        "private-over-budget",
        &payload("0", "0", "Closed"),
        false,
        "B",
    );
    s.open()
        .apply_authorized(&s.plans[0], &filtered, &authority(&s))
        .unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    let mut archive = batch(
        &s,
        30,
        "private-over-budget",
        &payload("0", "0", "Closed"),
        false,
        "B",
    );
    archive.events[0].id = "archive-over-budget".into();
    archive.events[0].witnesses.clear();
    archive.events[0].action = Action::ArchiveByPriorContract;
    s.open()
        .apply_authorized(&s.plans[0], &archive, &authority(&s))
        .unwrap();
    assert_eq!(read(&s, &binding).unwrap()["contracts"], "128");
    let enormous = serde_json::to_value(Value::RawJson(
        json!({"oversized":"x".repeat(MAX_BYTES)}).to_string(),
    ))
    .unwrap();
    s.open().client.execute("UPDATE cantongrid.contracts SET payload=$2 WHERE stream_key=$1 AND contract_id='private-0'",&[&stream_key(&s.plans[0]).0,&enormous]).unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
}
