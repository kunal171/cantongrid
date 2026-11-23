//! Real owned PostgreSQL with synthetic grants and lifecycle evidence; no live Canton acceptance.
use super::*;
use crate::model_projection::tests::{Scenario, lease};
use cantongrid_core::{
    lifecycle::{Action, Batch, Origin},
    visibility::PartyId,
};
use serde_json::{Value as Json, json};

/// Add a real filter so excluded canonical inputs and extra contributions are tested together.
fn scenario() -> Scenario {
    let mut document: Json = serde_json::from_str(include_str!(
        "../../../assets/reconciliation/backing-model.json"
    ))
    .unwrap();
    document["models"][0]["fields"]["state"] =
        json!({"op":"field","path":["state"],"as":{"type":"text"}});
    document["models"][0]["filter"] =
        json!({"field":"state","equals":{"type":"text","value":"Ready"}});
    let mut s = Scenario::new(&document.to_string());
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [s.models[0].template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    s
}
/// Only fully acknowledged live intervals can anchor a reconciliation read.
fn position(boundary: i64) -> String {
    json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX})
        .to_string()
}
/// Use small exact fractions; their last decimal digit decides the backing comparison.
fn payload(free: &str, locked: &str, state: &str) -> String {
    json!({"private_balances":{"free":free,"locked":locked,"payout":"0.1","contracts":"3"},"state":state}).to_string()
}
/// A fixture's witness clause is independent of numeric payload fields or caller-supplied names.
fn batch(
    s: &Scenario,
    boundary: i64,
    id: &str,
    payload: &str,
    snapshot: bool,
    witness: &str,
) -> Batch {
    let mut b = s.create(&position(boundary), id, payload);
    b.events[0].origin = Some(Origin {
        offset: boundary,
        snapshot_observation: snapshot,
    });
    b.events[0].witnesses = [PartyId::new(witness).unwrap()].into();
    b
}
/// Source authority includes both configured parties; callers must still authenticate independently.
fn authority(s: &Scenario) -> SourceLease {
    lease(&s.plans[0], vec!["A", "B"])
}
/// Build synthetic authenticated caller grants for deterministic visibility and denial controls.
fn caller(s: &Scenario, parties: &[&str]) -> CallerGrants {
    CallerGrants::from_verified(
        s.plans[0].context().binding(),
        parties.iter().map(|p| PartyId::new(*p).unwrap()),
    )
}
/// Policies are pure packaged inputs; they do not supply source coverage or permissions.
fn policy() -> Policy {
    Policy::parse(include_str!(
        "../../../assets/reconciliation/backing-policy.json"
    ))
    .unwrap()
}
/// Activate the installed model, then obtain the trusted current binding through existing discovery.
fn bind(s: &Scenario) -> BoundModel {
    s.open()
        .bind_model(
            &s.plans[0],
            &authority(s),
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
    s.open()
        .bound_models(&s.plans[0], &caller(s, &["A", "B"]), &authority(s))
        .unwrap()
        .remove(0)
}
/// Read through the public storage interface and inspect only its sanitized serialized report.
fn read(s: &Scenario, binding: &BoundModel) -> Result<Json, StoreError> {
    s.open()
        .read_reconciliation(
            &s.plans[0],
            &caller(s, &["A", "B"]),
            &authority(s),
            binding,
            &policy(),
        )
        .map(|r| serde_json::to_value(r).unwrap())
}
/// The initial snapshot-forward baseline is complete at offset ten, including both witness clauses.
fn seed(s: &Scenario) {
    let mut initial = batch(
        s,
        10,
        "private-contract-balanced",
        &payload("0.1", "0.2", "Ready"),
        true,
        "A",
    );
    initial.events.extend(
        batch(
            s,
            10,
            "private-contract-violated",
            &payload("0.1", "0.1999999999", "Ready"),
            true,
            "B",
        )
        .events,
    );
    initial.events.extend(
        batch(
            s,
            10,
            "private-contract-filtered",
            &payload("0", "0", "Closed"),
            true,
            "B",
        )
        .events,
    );
    s.open()
        .apply_authorized(&s.plans[0], &initial, &authority(s))
        .unwrap();
}

/// Exact comparisons, filtering, archives and replay share the model's transactional current state.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn indexed_reconciliation_tracks_exact_lifecycle_and_never_returns_private_rows() {
    let s = scenario();
    seed(&s);
    let binding = bind(&s);
    let result = read(&s, &binding).unwrap();
    assert_eq!(result["status"], "evaluated");
    assert_eq!(result["comparison"], "violated");
    assert_eq!(result["contracts"], "2");
    assert_eq!(result["checks"][0]["satisfied"], "1");
    assert_eq!(result["checks"][0]["violated"], "1");
    assert_eq!(result["completeWithinConfiguredScope"], true);
    assert_eq!(result["coverage"], "retained_indexed_prefix");
    assert_eq!(
        result["definitionFingerprint"],
        policy()
            .preflight_model(&s.namespace, &binding.model)
            .unwrap()
            .fingerprint()
    );
    let text = result.to_string();
    for private in [
        "private-contract",
        "private_balances",
        "0.1999999999",
        "sourceOffset",
        "witness",
        "boundary",
    ] {
        assert!(!text.contains(private));
    }
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
    let mut store = s.open();
    assert!(
        store
            .apply_authorized(&s.plans[0], &later, &authority(&s))
            .unwrap()
    );
    let after = read(&s, &binding).unwrap();
    assert_eq!(after["comparison"], "satisfied");
    assert_eq!(after["contracts"], "1");
    assert!(
        !store
            .apply_authorized(&s.plans[0], &later, &authority(&s))
            .unwrap()
    );
    assert_eq!(after, read(&s, &binding).unwrap());
    // Consumer reads must not manufacture ingestion freshness.
    let key = stream_key(&s.plans[0]).0;
    let before: String = store
        .client
        .query_one(
            "SELECT observed_at::text FROM cantongrid.streams WHERE stream_key=$1",
            &[&key],
        )
        .unwrap()
        .get(0);
    read(&s, &binding).unwrap();
    let after: String = store
        .client
        .query_one(
            "SELECT observed_at::text FROM cantongrid.streams WHERE stream_key=$1",
            &[&key],
        )
        .unwrap()
        .get(0);
    assert_eq!(before, after);
}

/// Denial depends on configured scope, not on whether hidden inputs currently affect an equation.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn full_scope_authorization_precedes_empty_state_metadata_and_rows() {
    let s = scenario();
    seed(&s);
    let binding = bind(&s);
    for narrow in [&["A"][..], &["B"][..], &[][..]] {
        assert_eq!(
            s.open()
                .read_reconciliation(
                    &s.plans[0],
                    &caller(&s, narrow),
                    &authority(&s),
                    &binding,
                    &policy()
                )
                .unwrap_err(),
            StoreError::Authorization
        );
    }
    let mut revoked = authority(&s);
    revoked.invalidate();
    assert_eq!(
        s.open()
            .read_reconciliation(
                &s.plans[0],
                &caller(&s, &["A", "B"]),
                &revoked,
                &binding,
                &policy()
            )
            .unwrap_err(),
        StoreError::Authorization
    );
    for wrong in [
        lease(&s.plans[0], vec!["A"]),
        lease(&s.plans[0], vec!["A", "B", "C"]),
    ] {
        assert_eq!(
            s.open()
                .read_reconciliation(
                    &s.plans[0],
                    &caller(&s, &["A", "B"]),
                    &wrong,
                    &binding,
                    &policy()
                )
                .unwrap_err(),
            StoreError::Authorization
        );
    }
    // A deliberately invalid binding still cannot let a narrow caller reach registry discovery.
    let mut missing = binding.clone();
    missing.model.name = "absent_private_model".into();
    assert_eq!(
        s.open()
            .read_reconciliation(
                &s.plans[0],
                &caller(&s, &["A"]),
                &authority(&s),
                &missing,
                &policy()
            )
            .unwrap_err(),
        StoreError::Authorization
    );
    assert_eq!(
        read(&s, &missing).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
}

/// Native numeric equality catches drift; cardinality catches extra filtered and archived contributions.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn projection_missing_drift_and_extra_rows_fail_without_partial_counts() {
    let s = scenario();
    seed(&s);
    let binding = bind(&s);
    let key = stream_key(&s.plans[0]).0;
    let mut store = s.open();
    let relation = s.relation(0);
    store.client.execute(&format!("UPDATE {relation} SET free=free+0.0000000001 WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2"),&[&key,&"private-contract-balanced"]).unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    store.client.execute(&format!("UPDATE {relation} SET free=free-0.0000000001 WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2"),&[&key,&"private-contract-balanced"]).unwrap();
    store.client.execute(&format!("INSERT INTO {relation} SELECT \"_cg_stream_key\",'private-contract-filtered',contracts,free,locked,payout,state FROM {relation} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"='private-contract-balanced'"),&[&key]).unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
    store
        .client
        .execute(
            &format!(
                "DELETE FROM {relation} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2"
            ),
            &[&key, &"private-contract-filtered"],
        )
        .unwrap();
    store
        .client
        .execute(
            &format!(
                "DELETE FROM {relation} WHERE \"_cg_stream_key\"=$1 AND \"_cg_contract_id\"=$2"
            ),
            &[&key, &"private-contract-balanced"],
        )
        .unwrap();
    assert_eq!(
        read(&s, &binding).unwrap_err(),
        StoreError::ReconciliationUnavailable
    );
}
