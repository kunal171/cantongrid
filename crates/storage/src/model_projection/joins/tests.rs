//! Real owned-PostgreSQL join denial/update/budget matrix with synthetic source authority.
use super::*;
use crate::BoundModel;
use crate::model_projection::tests::{PAYLOAD, Scenario, create, lease};
use cantongrid_core::{
    lifecycle::{Action, Batch},
    value::Value,
    visibility::PartyId,
};
use cantongrid_models::{Filter, JoinKind, JoinProjection, Relationship, RelationshipTarget};

/// Isolated namespace and physical stream; Scenario removes only its own rows/tables on drop.
struct JoinFixture {
    scenario: Scenario,
    source: BoundModel,
    target: BoundModel,
    plan: Plan,
}
impl JoinFixture {
    /// Two different templates share an explicitly declared key; outputs contain no raw payloads.
    fn new() -> Self {
        let mut doc: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/models/visibility.json")).unwrap();
        doc["models"][0]["fields"]["enabled"] =
            serde_json::json!({"op":"field","path":["enabled"],"as":{"type":"bool"}});
        let mut s = Scenario::new(&doc.to_string());
        let target = s.models[0].clone();
        let mut source = target.clone();
        source.name = format!("{}_root", s.namespace);
        source.template.entity = "Factory".into();
        source.filter = Some(Filter {
            field: "state".into(),
            equals: Value::Text("Anchor".into()),
        });
        source.relationships = vec![Relationship {
            name: "children".into(),
            revision: 1,
            source_field: "label".into(),
            target: RelationshipTarget {
                model: target.name.clone(),
                revision: 1,
                field: "label".into(),
            },
            max_targets: 100,
            join: Some(JoinProjection {
                kind: JoinKind::Inner,
                source_fields: vec![
                    "label".into(),
                    "note".into(),
                    "sequence".into(),
                    "enabled".into(),
                ],
                target_fields: vec![
                    "amount".into(),
                    "note".into(),
                    "sequence".into(),
                    "enabled".into(),
                ],
            }),
        }];
        s.install(source.clone());
        s.plans[0] = Plan::pqs(
            s.plans[0].context().clone(),
            [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
            [target.template.clone(), source.template.clone()].into(),
        )
        .unwrap()
        .with_source_binding("a".repeat(64));
        let plan = s.plans[0].clone();
        let binding = |model: &cantongrid_models::Model| BoundModel {
            namespace: s.namespace.clone(),
            model: model.clone(),
            fingerprint: model.fingerprint().unwrap(),
        };
        let left = binding(&source);
        let right = binding(&target);
        Self {
            scenario: s,
            source: left,
            target: right,
            plan,
        }
    }
    /// Bind only after a real source prefix exists; both definitions remain immutable revisions.
    fn start(&self, store: &mut Store) {
        self.apply(store, &self.batch("root", "root", "A", true, None));
        for model in [&self.source, &self.target] {
            store
                .bind_model(
                    &self.plan,
                    &lease(&self.plan, vec!["A", "B"]),
                    &model.namespace,
                    &model.model.name,
                    1,
                    None,
                )
                .unwrap();
        }
    }
    /// Build separate transactions with exact big integers/numerics and independently scoped witnesses.
    fn batch(
        &self,
        position: &str,
        id: &str,
        parties: &str,
        root: bool,
        note: Option<&str>,
    ) -> Batch {
        let mut payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        payload["enabled"] = root.into();
        if root {
            payload["state"] = "Anchor".into();
        }
        if let Some(note) = note {
            payload["note"] = note.into();
        }
        let model = if root {
            &self.source.model
        } else {
            &self.target.model
        };
        let mut batch = create(&self.plan, model, position, id, &payload.to_string());
        batch.events[0].witnesses = parties
            .split(',')
            .map(|p| PartyId::new(p).unwrap())
            .collect();
        batch
    }
    /// Use the same source lease as indexing; no API caller can impersonate another party subset.
    fn apply(&self, store: &mut Store, batch: &Batch) {
        store
            .apply_authorized(&self.plan, batch, &lease(&self.plan, vec!["A", "B"]))
            .unwrap();
    }
    /// Exercise the public typed storage path rather than duplicating SQL visibility logic in tests.
    fn read(
        &self,
        store: &mut Store,
        parties: Vec<&str>,
        root: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ModelPage, StoreError> {
        let caller = CallerGrants::from_verified(
            self.plan.context().binding(),
            parties.into_iter().map(|p| PartyId::new(p).unwrap()),
        );
        store.read_join(
            &self.plan,
            &caller,
            &lease(&self.plan, vec!["A", "B"]),
            RelationshipQuery {
                source: &self.source,
                target: &self.target,
                name: "children",
                root,
                after,
                limit,
            },
        )
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; no Canton writes"]
fn inner_joins_require_every_input_and_only_visible_updates_invalidate_pages() {
    let f = JoinFixture::new();
    let mut store = f.scenario.open();
    f.start(&mut store);
    f.apply(&mut store, &f.batch("b", "b", "B", false, None));
    assert_eq!(
        f.read(&mut store, vec!["A"], "root", None, 1)
            .unwrap()
            .visible_total,
        "0"
    );
    assert_eq!(
        f.read(&mut store, vec!["B"], "root", None, 1).err(),
        Some(StoreError::RelationshipUnavailable)
    );
    assert_eq!(
        f.read(&mut store, vec!["B"], "missing", None, 1).err(),
        Some(StoreError::RelationshipUnavailable)
    );
    let shared = f.read(&mut store, vec!["A", "B"], "root", None, 1).unwrap();
    let item = &shared.items[0];
    assert_eq!(item["joinKind"], "inner");
    assert_eq!(item["relationship"], "declared");
    assert_eq!(item["sourceValues"]["sequence"], "9223372036854775807");
    assert_eq!(
        item["targetValues"]["amount"],
        "12345678901234567890.1234567891"
    );
    assert_eq!(item["targetValues"]["enabled"], false);
    assert_eq!(item["sourceValues"]["enabled"], true);
    assert!(item["targetValues"]["note"].is_null());
    assert!(item["targetValues"].get("owner").is_none());
    f.apply(&mut store, &f.batch("a1", "a1", "A", false, None));
    f.apply(&mut store, &f.batch("a2", "a2", "A", false, None));
    let first = f.read(&mut store, vec!["A"], "root", None, 1).unwrap();
    let cursor = first.next_after.as_deref().unwrap();
    let both = f.read(&mut store, vec!["A", "B"], "root", None, 1).unwrap();
    f.apply(
        &mut store,
        &f.batch("hidden", "00-hidden", "B", false, Some("hidden changed")),
    );
    assert_eq!(
        f.read(&mut store, vec!["A"], "root", Some(cursor), 1)
            .unwrap()
            .items[0]["targetContractId"],
        "a2"
    );
    assert_eq!(
        f.read(
            &mut store,
            vec!["A", "B"],
            "root",
            both.next_after.as_deref(),
            1
        )
        .err(),
        Some(StoreError::ModelQuery)
    );
    let visible = f.batch("visible", "a3", "A", false, Some("visible changed"));
    f.apply(&mut store, &visible);
    assert_eq!(
        f.read(&mut store, vec!["A"], "root", Some(cursor), 1).err(),
        Some(StoreError::ModelQuery)
    );
    let current = f.read(&mut store, vec!["A"], "root", None, 1).unwrap();
    f.apply(&mut store, &visible);
    f.read(
        &mut store,
        vec!["A"],
        "root",
        current.next_after.as_deref(),
        1,
    )
    .unwrap();
    let mut bad = f.batch("bad", "bad", "A", false, None);
    bad.events[0].action = Action::Create(Value::RawJson("{}".into()));
    assert!(matches!(
        store.apply_authorized(&f.plan, &bad, &lease(&f.plan, vec!["A", "B"])),
        Err(StoreError::ModelProjection)
    ));
    f.read(
        &mut store,
        vec!["A"],
        "root",
        current.next_after.as_deref(),
        1,
    )
    .unwrap();
    let mut archive = f.batch("archive", "a1", "A", false, None);
    archive.events[0].id = "archive-a1".into();
    archive.events[0].witnesses.clear();
    archive.events[0].action = Action::ArchiveByPriorContract;
    f.apply(&mut store, &archive);
    assert_eq!(
        f.read(
            &mut store,
            vec!["A"],
            "root",
            current.next_after.as_deref(),
            1
        )
        .err(),
        Some(StoreError::ModelQuery)
    );
    f.apply(
        &mut store,
        &f.batch("successor", "a-successor", "A", false, None),
    );
    assert_eq!(
        f.read(&mut store, vec!["A"], "root", None, 25)
            .unwrap()
            .visible_total,
        "3"
    );
    let relation = f.scenario.relation(0);
    store
        .client
        .batch_execute(&format!(
            "ALTER TABLE {relation} ADD COLUMN unexpected text"
        ))
        .unwrap();
    let drift = f.read(&mut store, vec!["A"], "root", None, 1);
    store
        .client
        .batch_execute(&format!("ALTER TABLE {relation} DROP COLUMN unexpected"))
        .unwrap();
    assert_eq!(drift.err(), Some(StoreError::ModelSchema));
}

#[test]
#[ignore = "requires owned PostgreSQL; no Canton writes"]
fn selected_join_budgets_cannot_be_poisoned_by_hidden_values() {
    let f = JoinFixture::new();
    let mut store = f.scenario.open();
    f.start(&mut store);
    let large = "x".repeat(60_000);
    for index in 0..36 {
        f.apply(
            &mut store,
            &f.batch(
                &format!("large-{index}"),
                &format!("large-{index}"),
                "B",
                false,
                Some(&large),
            ),
        );
    }
    assert_eq!(
        f.read(&mut store, vec!["A"], "root", None, 1)
            .unwrap()
            .visible_total,
        "0"
    );
    assert_eq!(
        f.read(&mut store, vec!["A", "B"], "root", None, 1).err(),
        Some(StoreError::Limit)
    );
    let none =
        CallerGrants::from_verified(f.plan.context().binding(), [PartyId::new("C").unwrap()]);
    assert_eq!(
        store
            .read_join(
                &f.plan,
                &none,
                &lease(&f.plan, vec!["A", "B"]),
                RelationshipQuery {
                    source: &f.source,
                    target: &f.target,
                    name: "children",
                    root: "root",
                    after: None,
                    limit: 1
                }
            )
            .err(),
        Some(StoreError::RelationshipUnavailable)
    );
}
