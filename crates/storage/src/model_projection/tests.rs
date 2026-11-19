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

pub(super) const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
const VISIBILITY: &str = include_str!("../../assets/models/visibility.json");
const OUTCOMIA: &str = include_str!("../../assets/models/outcomia.json");
pub(super) const PAYLOAD: &str = include_str!("../../assets/models/visibility-payload.json");
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Own every synthetic source/model identity so cleanup never deletes another stream's data.
pub(super) struct Scenario {
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
pub(super) fn lease(plan: &Plan, parties: Vec<&str>) -> SourceLease {
    SourceLease::from_verified(
        plan.context().binding(),
        parties.into_iter().map(|p| PartyId::new(p).unwrap()),
        Instant::now(),
        MAX_SOURCE_LEASE,
        Instant::now() + MAX_SOURCE_LEASE,
    )
    .unwrap()
}
pub(super) fn create(
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
pub(super) fn archive(s: &Scenario, position: &str, contract: &str) -> Batch {
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
