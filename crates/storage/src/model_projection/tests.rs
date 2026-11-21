//! Owned-PostgreSQL controls with synthetic source authority, not live Canton acceptance.
use super::*;
use crate::{Store, initialize_env};
use cantongrid_core::{
    authorization::MAX_SOURCE_LEASE,
    config::SourceContext,
    lifecycle::{Action, Batch, Event},
    value::Value,
    visibility::PartyId,
};
use cantongrid_models::{Catalog, Model};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
const VISIBILITY: &str = include_str!("../../assets/models/visibility.json");
const OUTCOMIA: &str = include_str!("../../assets/models/outcomia.json");
pub(crate) const PAYLOAD: &str = include_str!("../../assets/models/visibility-payload.json");
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Own every synthetic source/model identity so cleanup never deletes another stream's data.
pub(crate) struct Scenario {
    pub namespace: String,
    pub aliases: Vec<String>,
    pub models: Vec<Model>,
    pub plans: Vec<Plan>,
}
impl Scenario {
    pub fn new(example: &str) -> Self {
        initialize_env(VARIABLE).unwrap();
        let namespace = format!(
            "proj_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let mut model = Catalog::parse(example).unwrap().models()[0].clone();
        model.name = namespace.clone();
        let plan = Self::plan(&namespace, &model);
        let mut s = Self {
            aliases: vec![namespace.clone()],
            namespace,
            models: vec![],
            plans: vec![plan],
        };
        s.install(model);
        s
    }
    pub fn visibility() -> Self {
        Self::new(VISIBILITY)
    }
    pub fn plan(id: &str, model: &Model) -> Plan {
        Plan::pqs(
            SourceContext::new(id.into(), "synthetic".into(), 1).unwrap(),
            [PartyId::new("A").unwrap()].into(),
            [model.template.clone()].into(),
        )
        .unwrap()
        .with_source_binding("a".repeat(64))
    }
    pub fn install(&mut self, model: Model) {
        let catalog =
            Catalog::parse(&serde_json::json!({"version":1,"models":[model]}).to_string()).unwrap();
        let mut store = self.open();
        store.register_models(&self.namespace, &catalog).unwrap();
        store
            .install_model(&self.namespace, &model.name, model.revision)
            .unwrap();
        self.models.push(model);
    }
    pub fn open(&self) -> Store {
        Store::open_env(VARIABLE).unwrap()
    }
    pub fn lease(&self) -> SourceLease {
        lease(&self.plans[0], vec!["A"])
    }
    pub fn bind(&self, revision: u32, previous: Option<u32>) -> Result<ModelBinding, StoreError> {
        self.open().bind_model(
            &self.plans[0],
            &self.lease(),
            &self.namespace,
            &self.models[0].name,
            revision,
            previous,
        )
    }
    pub fn create(&self, position: &str, contract: &str, payload: &str) -> Batch {
        create(&self.plans[0], &self.models[0], position, contract, payload)
    }
    pub fn seed(&self) {
        self.open()
            .apply_authorized(
                &self.plans[0],
                &self.create("snapshot", "first", PAYLOAD),
                &self.lease(),
            )
            .unwrap();
    }
    pub fn relation(&self, index: usize) -> String {
        format!(
            "cantongrid_models.\"{}\"",
            self.models[index].sql_plan().unwrap().table
        )
    }
    pub fn count(&self, index: usize, plan: &Plan) -> i64 {
        self.open()
            .client
            .query_one(
                &format!(
                    "SELECT count(*) FROM {} WHERE \"_cg_stream_key\"=$1",
                    self.relation(index)
                ),
                &[&stream_key(plan).0],
            )
            .unwrap()
            .get(0)
    }
}

/// Mock trusted checker evidence; wider and narrower sets are tested explicitly below.
pub(crate) fn lease(plan: &Plan, parties: Vec<&str>) -> SourceLease {
    SourceLease::from_verified(
        plan.context().binding(),
        parties.into_iter().map(|p| PartyId::new(p).unwrap()),
        Instant::now(),
        MAX_SOURCE_LEASE,
        Instant::now() + MAX_SOURCE_LEASE,
    )
    .unwrap()
}
pub(crate) fn create(
    plan: &Plan,
    model: &Model,
    position: &str,
    contract: &str,
    payload: &str,
) -> Batch {
    Batch {
        context: plan.context().clone(),
        position: position.into(),
        events: vec![Event {
            id: format!("create-{contract}"),
            contract: contract.into(),
            template: model.template.clone(),
            witnesses: [PartyId::new("A").unwrap()].into(),
            observed_at_micros: 1,
            ledger_at_micros: None,
            origin: None,
            action: Action::Create(Value::RawJson(payload.into())),
        }],
    }
}
pub(crate) fn archive(s: &Scenario, position: &str, contract: &str) -> Batch {
    let mut batch = s.create(position, contract, PAYLOAD);
    batch.events[0].id = format!("archive-{contract}");
    batch.events[0].witnesses.clear();
    batch.events[0].action = Action::ArchiveByPriorContract;
    batch
}
impl Drop for Scenario {
    fn drop(&mut self) {
        let Ok(mut client) = cantongrid_postgres::connect_env(VARIABLE) else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let result = (|| -> Result<(), StoreError> {
            let mut tables = BTreeSet::new();
            for model in &self.models {
                tables.insert(model.sql_plan().unwrap().table);
            }
            for plan in &self.plans {
                let key = stream_key(plan).0;
                for table in &tables {
                    // Missing-table drift tests restore their relation before cleanup.
                    tx.execute(
                        &format!(
                            "DELETE FROM cantongrid_models.\"{table}\" WHERE \"_cg_stream_key\"=$1"
                        ),
                        &[&key],
                    )?;
                }
                for table in [
                    "model_rebuild_dirty",
                    "model_rebuilds",
                    "selection_upgrades",
                    "model_bindings",
                    "outbox",
                    "events",
                    "contracts",
                    "batches",
                    "streams",
                ] {
                    tx.execute(
                        &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                        &[&key],
                    )?;
                }
            }
            for namespace in &self.aliases {
                tx.execute(
                    "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
                    &[&namespace],
                )?;
                tx.execute(
                    "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                    &[&namespace],
                )?;
            }
            for table in tables {
                let referenced: bool = tx.query_one("SELECT EXISTS(SELECT 1 FROM cantongrid.model_installations i JOIN cantongrid.model_tables t USING(fingerprint) WHERE t.table_name=$1)", &[&table])?.get(0);
                if !referenced {
                    tx.batch_execute(&format!("DROP TABLE cantongrid_models.\"{table}\""))?;
                    tx.execute(
                        "DELETE FROM cantongrid.model_tables WHERE table_name=$1",
                        &[&table],
                    )?;
                }
            }
            Ok(())
        })();
        if result.is_ok() {
            let _ = tx.commit();
        }
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn backfill_and_live_lifecycle_preserve_exact_values_for_both_examples() {
    for (example, payload, column, exact) in [
        (
            VISIBILITY,
            PAYLOAD,
            "amount",
            "12345678901234567890.1234567891",
        ),
        (
            OUTCOMIA,
            include_str!("../../assets/models/outcomia-payload.json"),
            "priceTickBps",
            "0.0000010000",
        ),
    ] {
        let s = Scenario::new(example);
        let plan = &s.plans[0];
        let mut store = s.open();
        let initial = s.create("snapshot", "first", payload);
        store.apply_authorized(plan, &initial, &s.lease()).unwrap();
        let result = s.bind(1, None).unwrap();
        assert_eq!((result.input_rows, result.projected_rows), (1, 1));
        let row = store
            .client
            .query_one(
                &format!("SELECT \"{column}\"::text FROM {}", s.relation(0)),
                &[],
            )
            .unwrap();
        assert_eq!(row.get::<_, String>(0), exact);
        assert!(!s.bind(1, None).unwrap().changed);
        assert!(!store.apply_authorized(plan, &initial, &s.lease()).unwrap());
        let future = s.create("future", "second", payload);
        store
            .apply_page(plan, &future, &s.lease(), Some("snapshot"))
            .unwrap();
        assert_eq!(s.count(0, plan), 2);
        store
            .apply_authorized(plan, &archive(&s, "archive", "first"), &s.lease())
            .unwrap();
        assert_eq!(s.count(0, plan), 1);
        assert_eq!(store.checkpoint(plan).unwrap().as_deref(), Some("archive"));
        assert!(!store.apply_authorized(plan, &future, &s.lease()).unwrap());
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn projection_failure_rolls_back_every_canonical_and_model_write() {
    let s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let mut store = s.open();
    let before = store.stats(&s.plans[0]).unwrap();
    let mut batch = s.create("bad-page", "good-before-failure", PAYLOAD);
    let invalid = PAYLOAD.replace("12345678901234567890.1234567891", "PRIVATE-invalid-amount");
    batch
        .events
        .extend(s.create("bad-page", "bad", &invalid).events);
    assert_eq!(
        store.apply_authorized(&s.plans[0], &batch, &s.lease()),
        Err(StoreError::ModelProjection)
    );
    assert_eq!(store.stats(&s.plans[0]).unwrap(), before);
    assert_eq!(
        store.checkpoint(&s.plans[0]).unwrap().as_deref(),
        Some("snapshot")
    );
    assert_eq!(s.count(0, &s.plans[0]), 1);
    assert!(!StoreError::ModelProjection.to_string().contains("PRIVATE"));
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn filters_nulls_and_archive_row_drift_are_checked() {
    let s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let mut store = s.open();
    let hidden = PAYLOAD.replace("Ready", "Draft");
    store
        .apply_authorized(
            &s.plans[0],
            &s.create("hidden", "hidden", &hidden),
            &s.lease(),
        )
        .unwrap();
    assert_eq!(s.count(0, &s.plans[0]), 1);
    let note: Option<String> = store
        .client
        .query_one(&format!("SELECT note FROM {}", s.relation(0)), &[])
        .unwrap()
        .get(0);
    assert_eq!(note, None);
    store
        .apply_authorized(
            &s.plans[0],
            &archive(&s, "hidden-archive", "hidden"),
            &s.lease(),
        )
        .unwrap();
    store
        .client
        .batch_execute(&format!("DELETE FROM {}", s.relation(0)))
        .unwrap();
    let before = store.stats(&s.plans[0]).unwrap();
    assert_eq!(
        store.apply_authorized(
            &s.plans[0],
            &archive(&s, "missing-row", "first"),
            &s.lease()
        ),
        Err(StoreError::ModelCorrupt)
    );
    assert_eq!(store.stats(&s.plans[0]).unwrap(), before);
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn schema_drift_stops_live_and_idle_progress_without_repair() {
    let s = Scenario::visibility();
    s.seed();
    s.bind(1, None).unwrap();
    let mut store = s.open();
    store
        .client
        .batch_execute(&format!(
            "ALTER TABLE {} ADD COLUMN unwanted text",
            s.relation(0)
        ))
        .unwrap();
    assert_eq!(
        store.observe_authorized(&s.plans[0], &s.lease()),
        Err(StoreError::ModelSchema)
    );
    assert_eq!(
        store.apply_authorized(
            &s.plans[0],
            &s.create("drift", "second", PAYLOAD),
            &s.lease()
        ),
        Err(StoreError::ModelSchema)
    );
    assert_eq!(
        store.checkpoint(&s.plans[0]).unwrap().as_deref(),
        Some("snapshot")
    );
    assert_eq!(s.bind(1, None), Err(StoreError::ModelSchema));
}

#[test]
#[ignore = "requires owned PostgreSQL; synthetic authority; run serially"]
fn model_pages_authorize_before_counts_and_reject_stale_revisions() {
    use cantongrid_core::visibility::CallerGrants;
    let mut s = Scenario::visibility();
    s.plans[0] = Plan::pqs(
        s.plans[0].context().clone(),
        [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
        s.plans[0].templates().cloned().collect(),
    )
    .unwrap()
    .with_source_binding("a".repeat(64));
    let source = lease(&s.plans[0], vec!["A", "B"]);
    let mut batch = s.create("snapshot", "a-only", PAYLOAD);
    let mut private = s.create("snapshot", "b-only", PAYLOAD).events.remove(0);
    private.witnesses = [PartyId::new("B").unwrap()].into();
    batch.events.push(private);
    let mut store = s.open();
    store
        .apply_authorized(&s.plans[0], &batch, &source)
        .unwrap();
    store
        .bind_model(
            &s.plans[0],
            &source,
            &s.namespace,
            &s.models[0].name,
            1,
            None,
        )
        .unwrap();
    let caller = |parties: Vec<&str>| {
        CallerGrants::from_verified(
            s.plans[0].context().binding(),
            parties.into_iter().map(|p| PartyId::new(p).unwrap()),
        )
    };
    let a = caller(vec!["A"]);
    let b = caller(vec!["B"]);
    let ab = caller(vec!["A", "B"]);
    let models = store.bound_models(&s.plans[0], &a, &source).unwrap();
    let first = store
        .read_model(&s.plans[0], &a, &source, &models[0], None, 1)
        .unwrap();
    assert_eq!(first.visible_total, "1");
    assert!(first.next_after.is_none());
    assert_eq!(first.items[0]["contractId"], "a-only");
    assert_eq!(first.items[0]["values"]["sequence"], "9223372036854775807");
    assert_eq!(
        first.items[0]["values"]["amount"],
        "12345678901234567890.1234567891"
    );
    assert!(first.items[0]["values"]["note"].is_null());
    let other = store
        .read_model(&s.plans[0], &b, &source, &models[0], None, 1)
        .unwrap();
    assert_eq!(other.visible_total, "1");
    assert_eq!(other.items[0]["contractId"], "b-only");
    let first = store
        .read_model(&s.plans[0], &ab, &source, &models[0], None, 1)
        .unwrap();
    assert_eq!(first.visible_total, "2");
    let next = store
        .read_model(
            &s.plans[0],
            &ab,
            &source,
            &models[0],
            first.next_after.as_deref(),
            1,
        )
        .unwrap();
    assert_eq!(next.items[0]["contractId"], "b-only");
    assert!(next.next_after.is_none());
    let mut revision = s.models[0].clone();
    revision.revision = 2;
    s.install(revision);
    store
        .bind_model(
            &s.plans[0],
            &source,
            &s.namespace,
            &s.models[0].name,
            2,
            Some(1),
        )
        .unwrap();
    assert!(matches!(
        store.read_model(&s.plans[0], &a, &source, &models[0], None, 1),
        Err(StoreError::ModelBinding)
    ));
}

/// Native numeric ordering, null placement and tie breakers must survive every page boundary.
#[test]
#[ignore = "requires separately owned disposable PostgreSQL destination"]
fn model_filters_sort_and_query_bound_keysets() {
    use cantongrid_core::visibility::CallerGrants;
    use cantongrid_models::QueryOptions;
    let s = Scenario::visibility();
    let mut store = s.open();
    let source = s.lease();
    let mut batch = s.create("snapshot", "a", PAYLOAD);
    batch.events.clear();
    for (contract, amount, note) in [
        ("a", "10.0", Some("same")),
        ("b", "2.0", None),
        ("c", "10.0", Some("same")),
        ("d", "10000000000000000000.0", None),
    ] {
        let mut payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        payload["amount"] = amount.into();
        payload["sequence"] = match contract {
            "a" => "30",
            "b" => "20",
            "c" => "10",
            _ => "40",
        }
        .into();
        payload["note"] = serde_json::json!(note);
        batch.events.push(
            s.create("unused", contract, &payload.to_string())
                .events
                .remove(0),
        );
    }
    store
        .apply_authorized(&s.plans[0], &batch, &source)
        .unwrap();
    s.bind(1, None).unwrap();
    let caller =
        CallerGrants::from_verified(s.plans[0].context().binding(), [PartyId::new("A").unwrap()]);
    let binding = store
        .bound_models(&s.plans[0], &caller, &source)
        .unwrap()
        .remove(0);
    let options = |value| serde_json::from_value::<QueryOptions>(value).unwrap();
    let query = options(
        serde_json::json!({"filters":{"amount":{"gte":"2.00","lt":"10000000000000000000.0"}},"sort":{"field":"amount"}}),
    );
    let first = store
        .read_model_query(&s.plans[0], &caller, &source, &binding, &query, None, 1)
        .unwrap();
    assert_eq!(first.visible_total, "3");
    assert_eq!(first.items[0]["contractId"], "b");
    let second = store
        .read_model_query(
            &s.plans[0],
            &caller,
            &source,
            &binding,
            &query,
            first.next_after.as_deref(),
            1,
        )
        .unwrap();
    assert_eq!(second.items[0]["contractId"], "a");
    let third = store
        .read_model_query(
            &s.plans[0],
            &caller,
            &source,
            &binding,
            &query,
            second.next_after.as_deref(),
            1,
        )
        .unwrap();
    assert_eq!(third.items[0]["contractId"], "c");
    assert!(third.next_after.is_none());
    let changed = options(serde_json::json!({"sort":{"field":"amount","direction":"desc"}}));
    assert!(matches!(
        store.read_model_query(
            &s.plans[0],
            &caller,
            &source,
            &binding,
            &changed,
            first.next_after.as_deref(),
            1
        ),
        Err(StoreError::ModelQuery)
    ));
    let page = store
        .read_model_query(&s.plans[0], &caller, &source, &binding, &changed, None, 4)
        .unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|r| r["contractId"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["d", "a", "c", "b"]
    );
    for direction in ["asc", "desc"] {
        let query = options(serde_json::json!({"sort":{"field":"note","direction":direction}}));
        let mut after = None;
        let mut ids = vec![];
        loop {
            let page = store
                .read_model_query(
                    &s.plans[0],
                    &caller,
                    &source,
                    &binding,
                    &query,
                    after.as_deref(),
                    1,
                )
                .unwrap();
            ids.push(page.items[0]["contractId"].as_str().unwrap().to_owned());
            after = page.next_after;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(ids, ["a", "c", "b", "d"]);
    }
    // Mixed directions and nullable ties must resume at the complete tuple, not only its first key.
    for (keys, expected) in [
        (
            serde_json::json!([{"field":"amount","direction":"desc"},{"field":"sequence","direction":"asc"}]),
            vec!["d", "c", "a", "b"],
        ),
        (
            serde_json::json!([{"field":"note"},{"field":"amount","direction":"desc"}]),
            vec!["a", "c", "d", "b"],
        ),
        (
            serde_json::json!([{"field":"note","direction":"desc"},{"field":"amount"}]),
            vec!["a", "c", "b", "d"],
        ),
    ] {
        let query = options(serde_json::json!({"sort":keys}));
        let mut after = None;
        let mut ids = Vec::new();
        loop {
            let page = store
                .read_model_query(
                    &s.plans[0],
                    &caller,
                    &source,
                    &binding,
                    &query,
                    after.as_deref(),
                    1,
                )
                .unwrap();
            ids.push(page.items[0]["contractId"].as_str().unwrap().to_owned());
            after = page.next_after;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(ids, expected);
    }
    let nulls = options(serde_json::json!({"filters":{"note":{"isNull":true}}}));
    let page = store
        .read_model_query(&s.plans[0], &caller, &source, &binding, &nulls, None, 4)
        .unwrap();
    assert_eq!(page.visible_total, "2");
}
