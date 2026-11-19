//! Real DDL, metadata and rollback controls against the disposable owned destination.
use super::*;
use cantongrid_core::value::{Decimal, Value};
use cantongrid_models::{Catalog, Expression, Model};
use std::sync::atomic::{AtomicU64, Ordering};

const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Unique model names produce unique physical tables. Cleanup never drops a table still linked
/// from another namespace, and never resets the destination or any application volume.
struct Scenario {
    namespaces: Vec<String>,
    model: Model,
}
impl Scenario {
    fn new(mut model: Model) -> Self {
        let mut client = cantongrid_postgres::connect_env(VARIABLE).unwrap();
        let fixture: bool = client
            .query_one(
                "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(
            fixture,
            "installation tests require the disposable owned destination"
        );
        crate::initialize_env(VARIABLE).unwrap();
        let suffix = format!(
            "{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        model.name = format!("install_{suffix}");
        let scenario = Self {
            namespaces: vec![format!("install_test_{suffix}")],
            model,
        };
        scenario
            .open()
            .register_models(&scenario.namespaces[0], &scenario.catalog())
            .unwrap();
        scenario
    }
    fn visibility() -> Self {
        Self::new(
            Catalog::parse(include_str!("../../assets/models/visibility.json"))
                .unwrap()
                .models()[0]
                .clone(),
        )
    }
    fn catalog(&self) -> Catalog {
        Catalog::parse(&serde_json::json!({"version":1,"models":[self.model]}).to_string()).unwrap()
    }
    fn open(&self) -> Store {
        Store::open_env(VARIABLE).unwrap()
    }
    fn install(&self) -> Result<ModelInstallation, StoreError> {
        self.open()
            .install_model(&self.namespaces[0], &self.model.name, self.model.revision)
    }
    fn relation(&self) -> String {
        format!(
            "cantongrid_models.\"{}\"",
            self.model.sql_plan().unwrap().table
        )
    }
}
impl Drop for Scenario {
    fn drop(&mut self) {
        let Ok(mut client) = cantongrid_postgres::connect_env(VARIABLE) else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let plan = self.model.sql_plan().unwrap();
        for namespace in &self.namespaces {
            if tx
                .execute(
                    "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
                    &[&namespace],
                )
                .is_err()
            {
                return;
            }
        }
        let Ok(row) = tx.query_one(
            "SELECT count(*) FROM cantongrid.model_installations i
             JOIN cantongrid.model_tables t USING(fingerprint)
             WHERE t.fingerprint=$1 OR t.table_name=$2",
            &[&plan.fingerprint, &plan.table],
        ) else {
            return;
        };
        if row.get::<_, i64>(0) == 0 {
            let Ok(row) = tx.query_one(
                "SELECT (SELECT relkind::text FROM pg_class WHERE oid=to_regclass($1))",
                &[&self.relation()],
            ) else {
                return;
            };
            // A drift control may replace the test table with a view.
            let command = if row.get::<_, Option<String>>(0).as_deref() == Some("v") {
                "DROP VIEW"
            } else {
                "DROP TABLE"
            };
            if tx
                .batch_execute(&format!("{command} IF EXISTS {}", self.relation()))
                .is_err()
                || tx
                    .execute(
                        "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1 OR table_name=$2",
                        &[&plan.fingerprint, &plan.table],
                    )
                    .is_err()
            {
                return;
            }
        }
        // Round-trip controls use the primary test namespace as their synthetic stream key.
        if tx
            .execute(
                "DELETE FROM cantongrid.contracts WHERE stream_key=$1",
                &[&self.namespaces[0]],
            )
            .is_err()
            || tx
                .execute(
                    "DELETE FROM cantongrid.streams WHERE stream_key=$1",
                    &[&self.namespaces[0]],
                )
                .is_err()
        {
            return;
        }
        for namespace in &self.namespaces {
            if tx
                .execute(
                    "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                    &[&namespace],
                )
                .is_err()
            {
                return;
            }
        }
        let _ = tx.commit();
    }
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn installation_is_idempotent_and_reuses_only_verified_managed_tables() {
    let mut scenario = Scenario::visibility();
    let first = scenario.install().unwrap();
    assert!(first.created && first.linked);
    let mut store = scenario.open();
    let oid = schema::table_oid(&mut store.client, &scenario.model.sql_plan().unwrap()).unwrap();
    let again = scenario.install().unwrap();
    assert!(!again.created && !again.linked);
    assert_eq!(again.table, first.table);
    assert_eq!(
        schema::table_oid(&mut store.client, &scenario.model.sql_plan().unwrap()).unwrap(),
        oid
    );
    let alias = format!("{}_alias", scenario.namespaces[0]);
    scenario.namespaces.push(alias.clone());
    store.register_models(&alias, &scenario.catalog()).unwrap();
    let shared = store
        .install_model(&alias, &scenario.model.name, 1)
        .unwrap();
    assert!(!shared.created && shared.linked);
    assert_eq!(shared.fingerprint, first.fingerprint);
    assert_eq!(
        store
            .client
            .query_one(
                "SELECT count(*) FROM cantongrid.model_tables WHERE fingerprint=$1",
                &[&first.fingerprint]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        store
            .client
            .query_one(
                "SELECT count(*) FROM cantongrid.model_installations WHERE fingerprint=$1",
                &[&first.fingerprint]
            )
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn both_examples_install_exact_native_columns_and_enforce_parent_identity() {
    for (example, payload) in [
        (
            include_str!("../../assets/models/visibility.json"),
            include_str!("../../assets/models/visibility-payload.json"),
        ),
        (
            include_str!("../../assets/models/outcomia.json"),
            include_str!("../../assets/models/outcomia-payload.json"),
        ),
    ] {
        let mut model = Catalog::parse(example).unwrap().models()[0].clone();
        // Exercise SQL Numeric scale boundaries and bool, not just the examples' scale 10.
        model.fields.insert(
            "flag".into(),
            Expression::Literal {
                value: Value::Bool(true),
            },
        );
        model.fields.insert(
            "tiny".into(),
            Expression::Literal {
                value: Value::Numeric(Decimal {
                    scale: 37,
                    decimal: format!("0.{}1", "0".repeat(36)),
                }),
            },
        );
        model.fields.insert(
            "whole".into(),
            Expression::Literal {
                value: Value::Numeric(Decimal {
                    scale: 0,
                    decimal: "9".repeat(38),
                }),
            },
        );
        let scenario = Scenario::new(model);
        scenario.install().unwrap();
        let mut store = scenario.open();
        let fields = scenario.model.evaluate(payload).unwrap().unwrap();
        let row = scenario
            .model
            .row_json(&scenario.namespaces[0], "synthetic-parent", &fields)
            .unwrap();
        let insert = format!(
            "INSERT INTO {} SELECT * FROM jsonb_populate_record(NULL::{},$1)",
            scenario.relation(),
            scenario.relation()
        );
        let error = store.client.execute(&insert, &[&row]).unwrap_err();
        assert_eq!(error.code().map(|code| code.code()), Some("23503"));
        store
            .client
            .execute(
                "INSERT INTO cantongrid.streams(stream_key,definition) VALUES($1,'{}')",
                &[&scenario.namespaces[0]],
            )
            .unwrap();
        store.client.execute("INSERT INTO cantongrid.contracts(stream_key,contract_id,template,payload,create_event_id,witnesses,active) VALUES($1,'synthetic-parent','{}','{}','test-create',ARRAY['A'],true)", &[&scenario.namespaces[0]]).unwrap();
        store.client.execute(&insert, &[&row]).unwrap();
        let values = store
            .client
            .query_one(
                &format!(
                    "SELECT \"flag\",\"tiny\"::text,\"whole\"::text FROM {}",
                    scenario.relation()
                ),
                &[],
            )
            .unwrap();
        assert!(values.get::<_, bool>(0));
        assert_eq!(values.get::<_, String>(1), format!("0.{}1", "0".repeat(36)));
        assert_eq!(values.get::<_, String>(2), "9".repeat(38));
        let error = store.client.execute(&insert, &[&row]).unwrap_err();
        assert_eq!(error.code().map(|code| code.code()), Some("23505"));
        assert!(!scenario.install().unwrap().created); // Retry preserves the populated table.
    }
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn unmanaged_tables_missing_revisions_and_metadata_drift_are_not_adopted_or_repaired() {
    let scenario = Scenario::visibility();
    let mut store = scenario.open();
    assert_eq!(
        store.install_model(&scenario.namespaces[0], &scenario.model.name, 2),
        Err(StoreError::ModelMissing)
    );
    schema::ensure_schema(&mut store.client).unwrap();
    let plan = scenario.model.sql_plan().unwrap();
    store.client.batch_execute(&plan.create_sql).unwrap();
    assert_eq!(scenario.install(), Err(StoreError::ModelSchema));
    assert_eq!(
        store
            .client
            .query_one(
                "SELECT count(*) FROM cantongrid.model_tables WHERE fingerprint=$1",
                &[&plan.fingerprint]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    store
        .client
        .batch_execute(&format!("DROP TABLE {}", scenario.relation()))
        .unwrap();
    scenario.install().unwrap();
    store
        .client
        .execute(
            "UPDATE cantongrid.model_tables SET ddl_sha256=repeat('0',64) WHERE fingerprint=$1",
            &[&plan.fingerprint],
        )
        .unwrap();
    assert_eq!(scenario.install(), Err(StoreError::ModelSchema));
    assert_eq!(
        store
            .client
            .query_one(
                "SELECT ddl_sha256 FROM cantongrid.model_tables WHERE fingerprint=$1",
                &[&plan.fingerprint]
            )
            .unwrap()
            .get::<_, String>(0),
        "0".repeat(64)
    );
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn changed_types_nullability_defaults_keys_indexes_and_public_grants_fail_closed() {
    for alteration in [
        "ALTER COLUMN \"amount\" TYPE numeric(38,9)",
        "ALTER COLUMN \"amount\" DROP NOT NULL",
        "ALTER COLUMN \"amount\" SET DEFAULT 0",
        "ALTER COLUMN \"label\" TYPE text COLLATE \"C\"",
        "ADD COLUMN extra text",
        "DROP COLUMN \"note\"",
        "ADD CONSTRAINT extra_unique UNIQUE (\"label\")",
        "ENABLE ROW LEVEL SECURITY",
    ] {
        let scenario = Scenario::visibility();
        scenario.install().unwrap();
        scenario
            .open()
            .client
            .batch_execute(&format!("ALTER TABLE {} {alteration}", scenario.relation()))
            .unwrap();
        assert_eq!(
            scenario.install(),
            Err(StoreError::ModelSchema),
            "drift control: {alteration}"
        );
    }
    for extra_index in [true, false] {
        let scenario = Scenario::visibility();
        scenario.install().unwrap();
        let sql = if extra_index {
            format!(
                "CREATE INDEX unexpected_index ON {} (\"label\")",
                scenario.relation()
            )
        } else {
            format!("GRANT SELECT ON {} TO PUBLIC", scenario.relation())
        };
        scenario.open().client.batch_execute(&sql).unwrap();
        assert_eq!(scenario.install(), Err(StoreError::ModelSchema));
    }
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn missing_tables_and_views_cannot_replace_a_managed_table() {
    for view in [false, true] {
        let scenario = Scenario::visibility();
        scenario.install().unwrap();
        let mut store = scenario.open();
        store
            .client
            .batch_execute(&format!("DROP TABLE {}", scenario.relation()))
            .unwrap();
        if view {
            store
                .client
                .batch_execute(&format!(
                    "CREATE VIEW {} AS SELECT 1 AS unexpected",
                    scenario.relation()
                ))
                .unwrap();
        }
        assert_eq!(scenario.install(), Err(StoreError::ModelSchema));
    }
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn actual_sql_failure_rolls_back_table_metadata_and_revision_link() {
    let scenario = Scenario::visibility();
    let mut store = scenario.open();
    let result =
        store.install_model_inner(&scenario.namespaces[0], &scenario.model.name, 1, |tx| {
            tx.batch_execute("SELECT 1/0")?;
            Ok(())
        });
    assert!(matches!(result, Err(StoreError::Database(_))));
    let plan = scenario.model.sql_plan().unwrap();
    assert_eq!(schema::table_oid(&mut store.client, &plan).unwrap(), None);
    for table in ["model_tables", "model_installations"] {
        let count: i64 = store
            .client
            .query_one(
                &format!("SELECT count(*) FROM cantongrid.{table} WHERE fingerprint=$1"),
                &[&plan.fingerprint],
            )
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
    }
    assert!(
        store
            .registered_model(&scenario.namespaces[0], &scenario.model.name, 1)
            .unwrap()
            .is_some()
    );
    assert!(scenario.install().unwrap().created);
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn global_installation_lock_prevents_competing_ddl() {
    let scenario = Scenario::visibility();
    let mut blocker = scenario.open();
    let mut tx = blocker.client.transaction().unwrap();
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&INSTALL_LOCK])
        .unwrap();
    assert_eq!(scenario.install(), Err(StoreError::Busy));
    tx.rollback().unwrap();
    assert!(scenario.install().unwrap().created);
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn foreign_key_actions_and_validation_cannot_drift_from_the_parent_contract() {
    for suffix in [
        "ON DELETE CASCADE",
        "DEFERRABLE INITIALLY DEFERRED",
        "NOT VALID",
    ] {
        let scenario = Scenario::visibility();
        scenario.install().unwrap();
        let mut store = scenario.open();
        let constraint: String = store.client.query_one(
            "SELECT conname::text FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='f'",
            &[&scenario.relation()],
        ).unwrap().get(0);
        // Quote the catalog name, including any embedded quote, before test-only DDL.
        let quoted = constraint.replace('"', "\"\"");
        store
            .client
            .batch_execute(&format!(
                "ALTER TABLE {} DROP CONSTRAINT \"{quoted}\";
             ALTER TABLE {} ADD CONSTRAINT changed_parent
             FOREIGN KEY (\"_cg_stream_key\",\"_cg_contract_id\")
             REFERENCES cantongrid.contracts(stream_key,contract_id) {suffix}",
                scenario.relation(),
                scenario.relation()
            ))
            .unwrap();
        assert_eq!(scenario.install(), Err(StoreError::ModelSchema));
    }
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn rules_triggers_inheritance_and_schema_public_access_are_rejected_without_repair() {
    let scenario = Scenario::visibility();
    scenario.install().unwrap();
    let plan = scenario.model.sql_plan().unwrap();
    let columns = scenario.model.sql_columns().unwrap();
    let relation = scenario.relation();
    let function = format!("cantongrid_models.\"{}_fn\"", plan.table);
    for ddl in [
        format!("CREATE RULE unexpected_rule AS ON UPDATE TO {relation} DO INSTEAD NOTHING"),
        format!("CREATE TABLE cantongrid_models.\"{}_child\" () INHERITS ({relation})", plan.table),
        format!("CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;
                 CREATE TRIGGER unexpected_trigger BEFORE INSERT ON {relation}
                 FOR EACH ROW EXECUTE FUNCTION {function}()"),
    ] {
        let mut store = scenario.open();
        let mut tx = store.client.transaction().unwrap();
        tx.batch_execute(&ddl).unwrap();
        assert_eq!(schema::verify_table(&mut tx, &plan, &columns), Err(StoreError::ModelSchema));
        tx.rollback().unwrap(); // No rule/function/child table escapes the control.
    }
    let mut store = scenario.open();
    let mut tx = store.client.transaction().unwrap();
    tx.batch_execute("GRANT USAGE ON SCHEMA cantongrid_models TO PUBLIC")
        .unwrap();
    assert_eq!(schema::ensure_schema(&mut tx), Err(StoreError::ModelSchema));
    let still_granted: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM pg_namespace n,
            LATERAL aclexplode(n.nspacl) acl
            WHERE n.nspname='cantongrid_models' AND acl.grantee=0
            AND acl.privilege_type='USAGE')",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(still_granted); // Failure did not silently rewrite an existing ACL.
    tx.rollback().unwrap();
    assert!(!scenario.install().unwrap().created);
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
fn conflicting_physical_identity_and_wrong_revision_links_fail_without_creating_tables() {
    let first = Scenario::visibility();
    let second = Scenario::visibility();
    let plan = first.model.sql_plan().unwrap();
    let mut store = first.open();
    let other_fingerprint = second.model.fingerprint().unwrap();
    store
        .client
        .execute(
            "INSERT INTO cantongrid.model_tables(fingerprint, table_name, plan_version, ddl_sha256)
         VALUES($1,$2,1,repeat('0',64))",
            &[&other_fingerprint, &plan.table],
        )
        .unwrap();
    assert_eq!(first.install(), Err(StoreError::ModelSchema));
    assert_eq!(schema::table_oid(&mut store.client, &plan).unwrap(), None);
    store
        .client
        .execute(
            "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1",
            &[&other_fingerprint],
        )
        .unwrap();
    first.install().unwrap();
    second.install().unwrap();
    store
        .client
        .execute(
            "UPDATE cantongrid.model_installations SET fingerprint=$1 WHERE namespace=$2",
            &[&other_fingerprint, &first.namespaces[0]],
        )
        .unwrap();
    let result = first.install();
    store
        .client
        .execute(
            "UPDATE cantongrid.model_installations SET fingerprint=$1 WHERE namespace=$2",
            &[&plan.fingerprint, &first.namespaces[0]],
        )
        .unwrap();
    assert_eq!(result, Err(StoreError::ModelSchema));
}
