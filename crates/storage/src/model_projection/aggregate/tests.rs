//! Real destination SQL with synthetic A/B authority; no LocalNet acceptance is implied.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, archive, lease};
use cantongrid_core::{config::SourceContext, lifecycle::Action, visibility::PartyId};
use serde_json::json;

/// Install a general-purpose exact-sum model and select both authorized source parties.
fn scenario() -> Scenario {
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/models/visibility.json")).unwrap();
    document["models"][0]["aggregation"] = json!({"group_by":["label"],"measures":{
        "contracts":{"op":"count"},"total":{"op":"sum","field":"amount"},
        "units":{"op":"sum","field":"sequence"}}});
    let mut s = Scenario::new(&document.to_string());
    s.plans[0] = Plan::pqs(
        SourceContext::new(s.namespace.clone(), "synthetic".into(), 1).unwrap(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        s.plans[0].templates().cloned().collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    s
}
fn source(s: &Scenario) -> SourceLease {
    lease(&s.plans[0], vec!["A", "B"])
}
fn caller(s: &Scenario, parties: &[&str]) -> CallerGrants {
    CallerGrants::from_verified(
        s.plans[0].context().binding(),
        parties.iter().map(|p| PartyId::new(*p).unwrap()),
    )
}
/// The fixture's owner payload does not confer visibility; only verified witnesses do.
fn add(s: &Scenario, position: &str, id: &str, label: &str, amount: &str, witnesses: &[&str]) {
    let mut value: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
    value["label"] = label.into();
    value["amount"] = amount.into();
    let mut batch = s.create(position, id, &value.to_string());
    batch.events[0].witnesses = witnesses
        .iter()
        .map(|p| PartyId::new(*p).unwrap())
        .collect();
    s.open()
        .apply_authorized(&s.plans[0], &batch, &source(s))
        .unwrap();
}
fn binding(s: &Scenario) -> BoundModel {
    s.open()
        .bound_models(&s.plans[0], &caller(s, &["A", "B"]), &source(s))
        .unwrap()
        .remove(0)
}
fn read(s: &Scenario, parties: &[&str], after: Option<&str>, limit: usize) -> ModelPage {
    s.open()
        .read_aggregate(
            &s.plans[0],
            &caller(s, parties),
            &source(s),
            &binding(s),
            after,
            limit,
        )
        .unwrap()
}
fn bind(s: &Scenario) {
    s.open()
        .bind_model(
            &s.plans[0],
            &source(s),
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic grants; run serially"]
fn aggregates_require_every_contributor_and_retract_without_replay_effects() {
    let s = scenario();
    add(&s, "1", "a", "mixed", "2.0000000000", &["A"]);
    add(&s, "2", "b", "mixed", "3.0000000000", &["B"]);
    add(&s, "3", "alone", "alone", "0.1000000000", &["A"]);
    add(&s, "4", "shared", "shared", "-0.1000000000", &["A", "B"]);
    bind(&s);
    for parties in [&["A"][..], &["B"][..]] {
        assert!(matches!(
            s.open().read_aggregate(
                &s.plans[0],
                &caller(&s, parties),
                &source(&s),
                &binding(&s),
                None,
                25
            ),
            Err(StoreError::Authorization)
        ));
    }
    let ab = read(&s, &["A", "B"], None, 25);
    assert_eq!(ab.visible_total, "3");
    let mixed = &ab
        .items
        .iter()
        .find(|r| r["values"]["label"] == "mixed")
        .unwrap()["values"];
    assert_eq!(mixed["contracts"], "2");
    assert_eq!(mixed["total"], "5.0000000000");
    assert_eq!(mixed["units"], "18446744073709551614");
    let first = read(&s, &["A", "B"], None, 1);
    let second = read(&s, &["A", "B"], first.next_after.as_deref(), 1);
    assert_ne!(first.items[0]["groupKey"], second.items[0]["groupKey"]);
    assert_eq!(second.visible_total, "3");
    let retired = archive(&s, "5", "b");
    assert!(
        s.open()
            .apply_authorized(&s.plans[0], &retired, &source(&s))
            .unwrap()
    );
    assert!(
        !s.open()
            .apply_authorized(&s.plans[0], &retired, &source(&s))
            .unwrap()
    );
    assert!(matches!(
        s.open().read_aggregate(
            &s.plans[0],
            &caller(&s, &["A"]),
            &source(&s),
            &binding(&s),
            None,
            25
        ),
        Err(StoreError::Authorization)
    ));
    let now = read(&s, &["A", "B"], None, 25);
    assert_eq!(now.visible_total, "3");
    let mixed = &now
        .items
        .iter()
        .find(|r| r["values"]["label"] == "mixed")
        .unwrap()["values"];
    assert_eq!(mixed["contracts"], "1");
    assert_eq!(mixed["total"], "2.0000000000");
    // A later invalid event must roll back a preceding create and its contribution.
    let before = read(&s, &["A", "B"], None, 25).items;
    let mut invalid = s.create("6", "uncommitted", PAYLOAD);
    let mut orphan = invalid.events[0].clone();
    orphan.id = "orphan-archive".into();
    orphan.contract = "missing".into();
    orphan.witnesses.clear();
    orphan.action = Action::ArchiveByPriorContract;
    invalid.events.push(orphan);
    assert!(
        s.open()
            .apply_authorized(&s.plans[0], &invalid, &source(&s))
            .is_err()
    );
    assert_eq!(before, read(&s, &["A", "B"], None, 25).items);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic grants; run serially"]
fn hidden_overflow_never_poisoned_visible_groups_and_sums_never_round() {
    let s = scenario();
    let max = "9999999999999999999999999999.9999999999";
    add(&s, "1", "secret1", "secret", max, &["B"]);
    add(&s, "2", "secret2", "secret", max, &["B"]);
    add(&s, "3", "visible", "visible", "0.0000000001", &["A"]);
    bind(&s);
    // Narrow callers are denied before looking at the dataset, even for overflow.
    assert!(matches!(
        s.open().read_aggregate(
            &s.plans[0],
            &caller(&s, &["A"]),
            &source(&s),
            &binding(&s),
            None,
            25
        ),
        Err(StoreError::Authorization)
    ));
    assert!(
        s.open()
            .read_aggregate(
                &s.plans[0],
                &caller(&s, &["A", "B"]),
                &source(&s),
                &binding(&s),
                None,
                25
            )
            .is_err()
    );
    // Simulate unusable visibility evidence in an owned database. The secondary
    // whole-group guard must hide that group before casting its overflowing sum.
    let mut client = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    let mut tx = client.transaction().unwrap();
    tx.execute("UPDATE cantongrid.contracts SET witnesses=ARRAY['C']::text[] WHERE stream_key=$1 AND contract_id IN ('secret1','secret2')",&[&crate::journal::stream_key(&s.plans[0]).0]).unwrap();
    tx.commit().unwrap();
    let full = read(&s, &["A", "B"], None, 25);
    assert_eq!(full.visible_total, "1");
    assert_eq!(full.items[0]["values"]["total"], "0.0000000001");
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic grants; run serially"]
fn aggregate_binding_changes_atomically_and_scope_mismatches_fail_closed() {
    use crate::RebuildAction;
    let mut s = scenario();
    add(&s, "1", "a", "mixed", "2.0000000000", &["A"]);
    add(&s, "2", "b", "mixed", "3.0000000000", &["B"]);
    bind(&s);
    let old = binding(&s);
    for parties in [vec!["A"], vec!["A", "B", "C"]] {
        assert!(matches!(
            s.open().read_aggregate(
                &s.plans[0],
                &caller(&s, &["A"]),
                &lease(&s.plans[0], parties),
                &old,
                None,
                25
            ),
            Err(StoreError::Authorization)
        ));
    }
    let mut revision = s.models[0].clone();
    revision.revision = 2;
    s.install(revision);
    s.open()
        .start_model_rebuild(
            &s.plans[0],
            &source(&s),
            "aggregate-v2",
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    let finish = |phase: &str| {
        for _ in 0..30 {
            let result = s
                .open()
                .model_rebuild(
                    &s.plans[0],
                    &source(&s),
                    "aggregate-v2",
                    RebuildAction::Step,
                    1,
                )
                .unwrap();
            if result.phase == phase {
                return;
            }
        }
        panic!("aggregate test rebuild did not reach {phase}");
    };
    finish("ready");
    assert_eq!(binding(&s).model.revision, 1);
    assert_eq!(
        read(&s, &["A", "B"], None, 25).items[0]["values"]["contracts"],
        "2"
    );
    s.open()
        .model_rebuild(
            &s.plans[0],
            &source(&s),
            "aggregate-v2",
            RebuildAction::Activate,
            1,
        )
        .unwrap();
    assert!(matches!(
        s.open().read_aggregate(
            &s.plans[0],
            &caller(&s, &["A", "B"]),
            &source(&s),
            &old,
            None,
            25
        ),
        Err(StoreError::ModelBinding)
    ));
    assert_eq!(binding(&s).model.revision, 2);
    assert_eq!(
        read(&s, &["A", "B"], None, 25).items[0]["values"]["total"],
        "5.0000000000"
    );
    finish("complete");
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic grants; run serially"]
fn unrelated_outcomia_counts_use_the_same_engine_in_a_single_party_view() {
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/models/outcomia.json")).unwrap();
    document["models"][0]["aggregation"] =
        json!({"group_by":["status"],"measures":{"markets":{"op":"count"}}});
    let s = Scenario::new(&document.to_string());
    let payload = include_str!("../../../assets/models/outcomia-payload.json");
    for (position, id, status) in [
        ("1", "closed1", "Closed"),
        ("2", "closed2", "Closed"),
        ("3", "open", "Open"),
    ] {
        let mut value: serde_json::Value = serde_json::from_str(payload).unwrap();
        value["status"] = status.into();
        s.open()
            .apply_authorized(
                &s.plans[0],
                &s.create(position, id, &value.to_string()),
                &s.lease(),
            )
            .unwrap();
    }
    s.bind(1, None).unwrap();
    let caller =
        CallerGrants::from_verified(s.plans[0].context().binding(), [PartyId::new("A").unwrap()]);
    let binding = s
        .open()
        .bound_models(&s.plans[0], &caller, &s.lease())
        .unwrap()
        .remove(0);
    let page = s
        .open()
        .read_aggregate(&s.plans[0], &caller, &s.lease(), &binding, None, 25)
        .unwrap();
    assert_eq!(page.visible_total, "2");
    assert_eq!(
        page.items[0]["values"],
        json!({"status":"Closed","markets":"2"})
    );
    assert_eq!(
        page.items[1]["values"],
        json!({"status":"Open","markets":"1"})
    );
}
