//! Owned PostgreSQL proofs and privacy controls use synthetic authority, not live Canton claims.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, lease};
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{
    config::Plan,
    exercise::{Exercise, ExercisePage},
    lifecycle::Origin,
    lineage::{Creation, LineagePage},
    visibility::{CallerGrants, PartyId},
};

/// Root is shared, endpoint contracts have independent policies; proof metadata discloses no payload.
fn page(s: &Scenario, position: &str, offset: i64, suffix: &str) -> LineagePage {
    let plan = &s.plans[0];
    let template = plan.templates().next().unwrap().clone();
    let mut batch = s.create(position, &format!("a{suffix}"), PAYLOAD);
    batch
        .events
        .extend(s.create(position, &format!("b{suffix}"), PAYLOAD).events);
    batch.events[1].witnesses = [PartyId::new("B").unwrap()].into();
    for e in &mut batch.events {
        e.origin = Some(Origin {
            offset,
            snapshot_observation: false,
        });
    }
    let exercise = Exercise {
        version: 1,
        offset,
        node_id: 0,
        transaction_id: position.into(),
        contract: "root".into(),
        template: template.clone(),
        choice: "Make".into(),
        consuming: false,
        argument: "{}".into(),
        result: "null".into(),
        controllers: ["operator".into()].into(),
        witnesses: ["A".into(), "B".into()].into(),
        ledger_at_micros: None,
        last_descendant_node_id: 2,
    };
    let creations = [(1, format!("a{suffix}")), (2, format!("b{suffix}"))]
        .into_iter()
        .map(|(node, contract)| Creation {
            version: 1,
            offset,
            node_id: node,
            transaction_id: position.into(),
            contract,
            template: template.clone(),
            witnesses: ["A".into(), "B".into()].into(),
            divulged_only: false,
        })
        .collect();
    LineagePage {
        delivery: ExercisePage {
            batch,
            exercises: vec![exercise],
        },
        creations,
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; run serially"]
fn lineage_visibility_replay_and_failed_commit_preserve_all_dependencies() {
    let mut s = Scenario::visibility();
    let old = s.plans[0].clone();
    s.plans[0] = Plan::pqs(
        old.context().clone(),
        ["A", "B", "C"]
            .into_iter()
            .map(|p| PartyId::new(p).unwrap())
            .collect(),
        old.templates().cloned().collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64))
    .with_exercises()
    .unwrap()
    .with_lineage()
    .unwrap();
    let plan = &s.plans[0];
    let authority = lease(plan, vec!["A", "B", "C"]);
    let mut store = s.open();
    let mut baseline = s.create("snapshot", "root", PAYLOAD);
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
    store
        .bind_model(plan, &authority, &s.namespace, &s.models[0].name, 1, None)
        .unwrap();
    let first = page(&s, "pair", 10, "");
    assert!(
        store
            .apply_lineage_page(plan, &first, &authority, Some("snapshot"))
            .unwrap()
    );
    assert!(
        !s.open()
            .apply_lineage_page(plan, &first, &authority, Some("pair"))
            .unwrap()
    );
    let caller = |names: &[&str]| {
        CallerGrants::from_verified(
            plan.context().binding(),
            names.iter().map(|p| PartyId::new(*p).unwrap()),
        )
    };
    for (who, child) in [(&["A"][..], "a"), (&["B"][..], "b")] {
        let visible = store
            .read_lineage(
                plan,
                &caller(who),
                &authority,
                LineageQuery {
                    root: "root",
                    direction: LineageDirection::Children,
                    after: None,
                    limit: 1,
                },
            )
            .unwrap();
        assert_eq!(visible.visible_total, 1);
        assert!(visible.next_after.is_none());
        assert_eq!(visible.items[0]["childContractId"], child);
        assert!(visible.items[0].get("visibility_clauses").is_none());
    }
    let both = store
        .read_lineage(
            plan,
            &caller(&["A", "B"]),
            &authority,
            LineageQuery {
                root: "root",
                direction: LineageDirection::Children,
                after: None,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(both.visible_total, 2);
    let next = store
        .read_lineage(
            plan,
            &caller(&["A", "B"]),
            &authority,
            LineageQuery {
                root: "root",
                direction: LineageDirection::Children,
                after: both.next_after.as_deref(),
                limit: 1,
            },
        )
        .unwrap();
    assert_ne!(both.items[0], next.items[0]);
    assert!(next.next_after.is_none());
    let parent = store
        .read_lineage(
            plan,
            &caller(&["A"]),
            &authority,
            LineageQuery {
                root: "a",
                direction: LineageDirection::Parents,
                after: None,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(parent.visible_total, 1);
    for root in ["b", "unknown"] {
        assert!(matches!(
            store.read_lineage(
                plan,
                &caller(&["A"]),
                &authority,
                LineageQuery {
                    root,
                    direction: LineageDirection::Parents,
                    after: None,
                    limit: 10
                }
            ),
            Err(StoreError::LineageUnavailable)
        ));
    }
    assert!(matches!(
        store.read_lineage(
            plan,
            &caller(&["C"]),
            &authority,
            LineageQuery {
                root: "root",
                direction: LineageDirection::Children,
                after: None,
                limit: 10
            }
        ),
        Err(StoreError::LineageUnavailable)
    ));

    // A hidden proof input suppresses the derived edge even though A can read both its endpoints.
    let mut second = page(&s, "second", 20, "2");
    second.creations[1].witnesses = ["B".into()].into();
    store
        .apply_lineage_page(plan, &second, &authority, Some("pair"))
        .unwrap();
    let a = store
        .read_lineage(
            plan,
            &caller(&["A"]),
            &authority,
            LineageQuery {
                root: "root",
                direction: LineageDirection::Children,
                after: None,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(a.visible_total, 1);
    assert!(a.next_after.is_none());
    assert_eq!(s.count(0, plan), 5);
    let key = stream_key(plan).0;
    let counts = |store: &mut Store| -> (i64, i64, i64) {
        let r=store.client.query_one("SELECT (SELECT count(*) FROM cantongrid.creation_events WHERE stream_key=$1),(SELECT count(*) FROM cantongrid.lineage_edges WHERE stream_key=$1),(SELECT count(*) FROM cantongrid.lineage_outbox WHERE stream_key=$1)",&[&key]).unwrap();
        (r.get(0), r.get(1), r.get(2))
    };
    let before = counts(&mut store);
    assert_eq!(before, (4, 4, 4));
    // An actual SQL failure after graph staging must roll back lifecycle/model/journal progress too.
    store.client.batch_execute("CREATE FUNCTION cantongrid.lineage_fail_test() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'controlled lineage failure'; END$$; CREATE TRIGGER lineage_fail_test BEFORE INSERT ON cantongrid.lineage_outbox FOR EACH ROW EXECUTE FUNCTION cantongrid.lineage_fail_test();").unwrap();
    let failed = store.apply_lineage_page(
        plan,
        &page(&s, "third", 30, "3"),
        &authority,
        Some("second"),
    );
    store.client.batch_execute("DROP TRIGGER lineage_fail_test ON cantongrid.lineage_outbox; DROP FUNCTION cantongrid.lineage_fail_test();").unwrap();
    assert!(failed.is_err());
    assert_eq!(store.checkpoint(plan).unwrap().as_deref(), Some("second"));
    assert_eq!(counts(&mut store), before);
    assert_eq!(s.count(0, plan), 5);
    let mut revoked = authority;
    // Expiry after derived edges/outbox have been staged must roll back the entire delivery.
    store.client.batch_execute("CREATE FUNCTION cantongrid.lineage_delay_test() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_sleep(1); RETURN NEW; END$$; CREATE TRIGGER lineage_delay_test BEFORE INSERT ON cantongrid.lineage_outbox FOR EACH ROW EXECUTE FUNCTION cantongrid.lineage_delay_test();").unwrap();
    let now = std::time::Instant::now();
    let expiring = cantongrid_core::authorization::SourceLease::from_verified(
        plan.context().binding(),
        plan.source_parties().cloned(),
        now,
        cantongrid_core::authorization::MAX_SOURCE_LEASE,
        now + std::time::Duration::from_millis(500),
    )
    .unwrap();
    let late = store.apply_lineage_page(
        plan,
        &page(&s, "late", 30, "late"),
        &expiring,
        Some("second"),
    );
    store.client.batch_execute("DROP TRIGGER lineage_delay_test ON cantongrid.lineage_outbox; DROP FUNCTION cantongrid.lineage_delay_test();").unwrap();
    assert_eq!(late, Err(StoreError::Authorization));
    assert_eq!(counts(&mut store), before);
    assert_eq!(s.count(0, plan), 5);
    assert_eq!(store.checkpoint(plan).unwrap().as_deref(), Some("second"));
    revoked.invalidate();
    assert!(matches!(
        store.read_lineage(
            plan,
            &caller(&["A"]),
            &revoked,
            LineageQuery {
                root: "root",
                direction: LineageDirection::Children,
                after: None,
                limit: 10
            }
        ),
        Err(StoreError::Authorization)
    ));
}
