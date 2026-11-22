//! Owned-PostgreSQL cross-transaction lifecycle/privacy controls; no Canton ledger writes.
use super::*;
use crate::model_projection::tests::{PAYLOAD, Scenario, create, lease};
use cantongrid_core::{lifecycle::Action, value::Value, visibility::PartyId};
use cantongrid_models::{Filter, Relationship, RelationshipTarget};

#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority"]
fn declared_links_follow_atomic_current_state_without_becoming_ancestry() {
    let mut s = Scenario::visibility();
    let target = s.models[0].clone();
    let mut source = target.clone();
    source.template.entity = "Factory".into();
    source.name = format!("{}_root", s.namespace);
    source.filter = Some(Filter {
        field: "state".into(),
        equals: Value::Text("Anchor".into()),
    });
    source.relationships = vec![
        Relationship {
            name: "children".into(),
            revision: 1,
            source_field: "label".into(),
            target: RelationshipTarget {
                model: target.name.clone(),
                revision: 1,
                field: "label".into(),
            },
            max_targets: 2,
        },
        Relationship {
            name: "amountPeers".into(),
            revision: 1,
            source_field: "amount".into(),
            target: RelationshipTarget {
                model: target.name.clone(),
                revision: 1,
                field: "amount".into(),
            },
            max_targets: 100,
        },
    ];
    s.install(source.clone());
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [target.template.clone(), source.template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    let plan = s.plans[0].clone();
    let authority = lease(&plan, vec!["A", "B"]);
    let make = |position: &str, id: &str, owner: &str, anchor: bool, amount: Option<&str>| {
        let mut payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        if anchor {
            payload["state"] = "Anchor".into();
        }
        if let Some(amount) = amount {
            payload["amount"] = amount.into();
        }
        let mut batch = create(&plan, &target, position, id, &payload.to_string());
        if anchor {
            batch.events[0].template = source.template.clone();
        }
        batch.events[0].witnesses = owner.split(',').map(|p| PartyId::new(p).unwrap()).collect();
        batch
    };
    let mut store = s.open();
    store
        .apply_authorized(&plan, &make("root", "root", "A,B", true, None), &authority)
        .unwrap();
    for model in [&target, &source] {
        store
            .bind_model(&plan, &authority, &s.namespace, &model.name, 1, None)
            .unwrap();
    }
    let binding = |model: &cantongrid_models::Model| BoundModel {
        namespace: s.namespace.clone(),
        model: model.clone(),
        fingerprint: model.fingerprint().unwrap(),
    };
    let mut changed = source.clone();
    changed.relationships[0].revision = 2;
    let conflict = cantongrid_models::Catalog::parse(
        &serde_json::json!({"version":1,"models":[changed]}).to_string(),
    )
    .unwrap();
    assert!(matches!(
        store.register_models(&s.namespace, &conflict),
        Err(StoreError::ModelConflict)
    ));
    let left = binding(&source);
    let right = binding(&target);
    let a = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("A").unwrap()]);
    let b = CallerGrants::from_verified(plan.context().binding(), [PartyId::new("B").unwrap()]);
    let ab = CallerGrants::from_verified(
        plan.context().binding(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()],
    );
    let read =
        |store: &mut Store, caller: &CallerGrants, name: &str, root: &str, after: Option<&str>| {
            store.read_relationship(
                &plan,
                caller,
                &authority,
                RelationshipQuery {
                    source: &left,
                    target: &right,
                    name,
                    root,
                    after,
                    limit: 1,
                },
            )
        };
    // Each child is a different committed batch, with no exercise or source-tree evidence.
    store
        .apply_authorized(&plan, &make("one", "a-child", "A", false, None), &authority)
        .unwrap();
    store
        .apply_authorized(&plan, &make("two", "b-child", "B", false, None), &authority)
        .unwrap();
    let first = read(&mut store, &ab, "children", "root", None).unwrap();
    assert_eq!(first.visible_total, "2");
    assert!(first.next_after.is_some());
    let second = read(
        &mut store,
        &ab,
        "children",
        "root",
        first.next_after.as_deref(),
    )
    .unwrap();
    assert_eq!(second.items[0]["targetContractId"], "b-child");
    assert!(second.next_after.is_none());
    for caller in [&a, &b] {
        let page = read(&mut store, caller, "children", "root", None).unwrap();
        assert_eq!(page.visible_total, "1");
        assert!(page.next_after.is_none());
    }
    assert_eq!(first.items[0]["relationship"], "declared");
    assert!(first.items[0].get("proof").is_none());
    assert!(first.items[0].get("values").is_none());
    assert!(first.items[0].get("label").is_none());
    // A hidden third match cannot cause A's bounded-fanout query to fail.
    store
        .apply_authorized(
            &plan,
            &make(
                "hidden",
                "00-hidden",
                "B",
                false,
                Some("12345678901234567890.1234567892"),
            ),
            &authority,
        )
        .unwrap();
    assert_eq!(
        read(&mut store, &a, "children", "root", None)
            .unwrap()
            .visible_total,
        "1"
    );
    assert!(matches!(
        read(&mut store, &ab, "children", "root", None),
        Err(StoreError::Limit)
    ));
    assert_eq!(
        read(&mut store, &ab, "amountPeers", "root", None)
            .unwrap()
            .visible_total,
        "2"
    );
    assert_eq!(
        read(&mut store, &a, "children", "b-child", None).err(),
        Some(StoreError::RelationshipUnavailable)
    );
    assert_eq!(
        read(&mut store, &a, "children", "unknown", None).err(),
        Some(StoreError::RelationshipUnavailable)
    );
    // Archive/recreate changes contract identity, while the explicit application key remains stable.
    let mut archive = make("archive", "a-child", "A", false, None);
    archive.events[0].id = "archive-a-child".into();
    archive.events[0].action = Action::ArchiveByPriorContract;
    archive.events[0].witnesses.clear();
    store.apply_authorized(&plan, &archive, &authority).unwrap();
    assert_eq!(
        read(&mut store, &a, "children", "root", None)
            .unwrap()
            .visible_total,
        "0"
    );
    let successor = make("successor", "a-successor", "A", false, None);
    store
        .apply_authorized(&plan, &successor, &authority)
        .unwrap();
    store
        .apply_authorized(&plan, &successor, &authority)
        .unwrap();
    let page = read(&mut store, &a, "children", "root", None).unwrap();
    assert_eq!(page.visible_total, "1");
    assert_eq!(page.items[0]["targetContractId"], "a-successor");
    // A bad projection rolls back canonical state and the checkpoint, preserving relationship reads.
    let mut bad = make("bad", "bad-child", "A", false, None);
    bad.events[0].action = Action::Create(Value::RawJson("{}".into()));
    assert!(matches!(
        store.apply_authorized(&plan, &bad, &authority),
        Err(StoreError::ModelProjection)
    ));
    assert_eq!(
        read(&mut store, &a, "children", "root", None)
            .unwrap()
            .visible_total,
        "1"
    );
    let mut stale = right.clone();
    stale.fingerprint = "0".repeat(64);
    assert!(matches!(
        store.read_relationship(
            &plan,
            &a,
            &authority,
            RelationshipQuery {
                source: &left,
                target: &stale,
                name: "children",
                root: "root",
                after: None,
                limit: 1
            }
        ),
        Err(StoreError::ModelBinding)
    ));
    let wrong = Plan::pqs(
        cantongrid_core::config::SourceContext::new("unrelated".into(), "synthetic".into(), 1)
            .unwrap(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        [target.template.clone()].into(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    assert!(
        store
            .read_relationship(
                &wrong,
                &a,
                &authority,
                RelationshipQuery {
                    source: &left,
                    target: &right,
                    name: "children",
                    root: "root",
                    after: None,
                    limit: 1
                }
            )
            .is_err()
    );
}
