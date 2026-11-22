//! Owned PostgreSQL diagnostics are read-only and never return committed payloads.
use crate::model_projection::tests::{PAYLOAD, Scenario, archive, lease};
use crate::{BoundModel, StoreError};
use cantongrid_core::visibility::{CallerGrants, PartyId};
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn projection_inspection_preserves_exact_comparison_and_never_repairs_drift() {
    let s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let plan = &s.plans[0];
    let mut store = s.open();
    let authority = lease(plan, vec!["A"]);
    let caller =
        CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let binding = BoundModel {
        namespace: s.namespace.clone(),
        model: s.models[0].clone(),
        fingerprint: s.models[0].fingerprint().unwrap(),
    };
    let original = store.checkpoint(plan).unwrap();
    let page = store
        .inspect_projection(plan, &caller, &authority, &binding, "first")
        .unwrap();
    assert_eq!(page.items[0]["projection"], "consistent");
    let encoded = serde_json::to_string(&page).unwrap();
    assert!(!encoded.contains("12345678901234567890"));
    assert!(!encoded.contains("example-record"));
    assert!(!encoded.contains("payload"));
    store
        .client
        .batch_execute(&format!(
            "UPDATE {} SET amount=amount+0.0000000001",
            s.relation(0)
        ))
        .unwrap();
    assert_eq!(
        store
            .inspect_projection(plan, &caller, &authority, &binding, "first")
            .unwrap()
            .items[0]["projection"],
        "different"
    );
    assert_eq!(store.checkpoint(plan).unwrap(), original);
    assert_eq!(
        store
            .inspect_projection(plan, &caller, &authority, &binding, "missing")
            .err(),
        Some(StoreError::InspectorUnavailable)
    );
    let other = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("B").unwrap()]);
    assert_eq!(
        store
            .inspect_projection(plan, &other, &authority, &binding, "first")
            .err(),
        Some(StoreError::InspectorUnavailable)
    );
    // Restore test-only drift before normal archival, which validates the canonical original payload.
    store
        .client
        .batch_execute(&format!(
            "UPDATE {} SET amount=amount-0.0000000001",
            s.relation(0)
        ))
        .unwrap();
    store
        .apply_authorized(plan, &archive(&s, "archive", "first"), &authority)
        .unwrap();
    let page = store
        .inspect_projection(plan, &caller, &authority, &binding, "first")
        .unwrap();
    assert_eq!(page.items[0]["projection"], "consistent");
    assert_eq!(page.items[0]["active"], false);
    let progress = store.inspect_progress(plan, &authority).unwrap();
    assert!(progress.get("destinationRevision").is_some());
    assert_eq!(progress["failedTrace"], "not_recorded");
    assert!(progress.get("checkpoint").is_none());
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn transaction_inspection_renumbers_visible_nodes_and_reconciles_only_indexed_evidence() {
    use cantongrid_core::{
        config::Plan,
        exercise::{Exercise, ExercisePage},
        lifecycle::{Action, Origin},
        lineage::{Creation, LineagePage},
    };
    let mut s = Scenario::visibility();
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        s.plans[0].templates().cloned().collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64))
    .with_exercises()
    .unwrap()
    .with_lineage()
    .unwrap();
    let plan = &s.plans[0];
    let authority = lease(plan, vec!["A", "B"]);
    let mut store = s.open();
    let mut baseline = s.create("baseline", "root", PAYLOAD);
    baseline.events[0].witnesses = [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into();
    baseline.events[0].origin = Some(Origin {
        offset: 5,
        snapshot_observation: true,
    });
    store
        .apply_lineage_page(
            plan,
            &LineagePage {
                delivery: ExercisePage {
                    batch: baseline,
                    exercises: vec![],
                },
                creations: vec![],
            },
            &authority,
            None,
        )
        .unwrap();
    let mut batch = s.create("known-transaction", "a", PAYLOAD);
    batch
        .events
        .extend(s.create("known-transaction", "b", PAYLOAD).events);
    batch.events[1].witnesses = [PartyId::new("B").unwrap()].into();
    batch
        .events
        .extend(s.create("known-transaction", "c", PAYLOAD).events);
    let mut archived = s
        .create("known-transaction", "root", PAYLOAD)
        .events
        .remove(0);
    archived.id = "archive-root".into();
    archived.action = Action::ArchiveByPriorContract;
    archived.witnesses.clear();
    batch.events.push(archived);
    for e in &mut batch.events {
        e.origin = Some(Origin {
            offset: 10,
            snapshot_observation: false,
        });
    }
    let template = s.models[0].template.clone();
    let make_exercise = |node, contract: &str, last| Exercise {
        version: 1,
        offset: 10,
        node_id: node,
        transaction_id: "known-transaction".into(),
        contract: contract.into(),
        template: template.clone(),
        choice: "Make".into(),
        consuming: true,
        argument: "{\"secret-sentinel\":true}".into(),
        result: "null".into(),
        controllers: ["secret-controller".into()].into(),
        witnesses: ["A".into(), "B".into()].into(),
        ledger_at_micros: None,
        last_descendant_node_id: last,
    };
    let creations = [
        (1, "a", false),
        (2, "b", false),
        (3, "secret-relay", true),
        (5, "secret-transient", true),
        (6, "c", false),
    ]
    .into_iter()
    .map(|(node, contract, divulged_only)| Creation {
        version: 1,
        offset: 10,
        node_id: node,
        transaction_id: "known-transaction".into(),
        contract: contract.into(),
        template: template.clone(),
        witnesses: ["A".into(), "B".into()].into(),
        divulged_only,
    })
    .collect();
    let page = LineagePage {
        delivery: ExercisePage {
            batch,
            exercises: vec![
                make_exercise(0, "root", 6),
                make_exercise(4, "secret-relay", 5),
            ],
        },
        creations,
    };
    store
        .apply_lineage_page(plan, &page, &authority, Some("baseline"))
        .unwrap();
    let checkpoint = store.checkpoint(plan).unwrap();
    for (parties, expected) in [(vec!["A"], 4), (vec!["B"], 3), (vec!["A", "B"], 5)] {
        let caller = CallerGrants::from_verified(
            plan.context().binding(),
            parties.into_iter().map(|p| PartyId::new(p).unwrap()),
        );
        let mut after = None;
        let mut items = Vec::new();
        loop {
            let response = store
                .inspect_transaction(
                    plan,
                    &caller,
                    &authority,
                    "known-transaction",
                    after.as_deref(),
                    1,
                )
                .unwrap();
            assert_eq!(response.visible_total, expected);
            items.extend(response.items);
            after = response.next_after;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(
            items
                .iter()
                .map(|v| v["ordinal"].as_str().unwrap().parse::<i64>().unwrap())
                .collect::<Vec<_>>(),
            (1..=expected).collect::<Vec<_>>()
        );
        assert_eq!(items.iter().filter(|v| v["kind"] == "exercise").count(), 1);
        assert_eq!(items.iter().filter(|v| v["kind"] == "archive").count(), 1);
        let encoded = serde_json::to_string(&items).unwrap();
        for forbidden in [
            "secret-",
            "node_id",
            "witness",
            "argument",
            "last_descendant",
            "source_offset",
            "example-record",
        ] {
            assert!(!encoded.contains(forbidden), "{forbidden}");
        }
    }
    let caller =
        CallerGrants::from_verified(plan.context().binding(), [PartyId::new("C").unwrap()]);
    assert_eq!(
        store
            .inspect_transaction(plan, &caller, &authority, "known-transaction", None, 20)
            .err(),
        Some(StoreError::InspectorUnavailable)
    );
    assert_eq!(
        store
            .inspect_transaction(plan, &caller, &authority, "missing", None, 20)
            .err(),
        Some(StoreError::InspectorUnavailable)
    );
    assert_eq!(store.checkpoint(plan).unwrap(), checkpoint);
    store
        .apply_lineage_page(plan, &page, &authority, checkpoint.as_deref())
        .unwrap();
    assert_eq!(store.checkpoint(plan).unwrap(), checkpoint);
}
