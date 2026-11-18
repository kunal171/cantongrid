//! Owned-Postgres controls use synthetic namespaces and delete only their own registry rows.
use super::*;
use crate::initialize_env;
use cantongrid_models::Expression;
use std::sync::atomic::{AtomicU64, Ordering};

const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Require the disposable destination before any fixture initialization or cleanup.
struct Scenario {
    namespace: String,
}
impl Scenario {
    fn new() -> Self {
        let mut client = cantongrid_postgres::connect_env(VARIABLE).unwrap();
        let owned: bool = client
            .query_one(
                "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(owned, "registry tests require the disposable owned fixture");
        initialize_env(VARIABLE).unwrap();
        Self {
            namespace: format!(
                "registry_test_{}_{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
        }
    }
    fn open(&self) -> Store {
        Store::open_env(VARIABLE).unwrap()
    }
}
impl Drop for Scenario {
    fn drop(&mut self) {
        if let Ok(mut client) = cantongrid_postgres::connect_env(VARIABLE) {
            // Test-only SQL cleanup, never a product update/delete interface.
            let _ = client.execute(
                "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                &[&self.namespace],
            );
        }
    }
}

fn visibility() -> Catalog {
    Catalog::parse(include_str!("../../../../examples/models/visibility.json")).unwrap()
}

/// Build a bounded catalog through the same strict entry point as a developer file.
fn catalog(models: &[Model]) -> Catalog {
    Catalog::parse(&serde_json::json!({"version":1,"models":models}).to_string()).unwrap()
}

#[test]
fn namespace_labels_are_bounded_and_never_become_sql() {
    for invalid in [
        "",
        "1demo",
        "_system",
        "demo'; DROP SCHEMA public",
        "has space",
        "é",
    ] {
        assert_eq!(
            validate_namespace(invalid),
            Err(StoreError::ModelDefinition)
        );
    }
    assert!(validate_namespace(&"x".repeat(65)).is_err());
    assert!(validate_namespace(&"x".repeat(64)).is_ok());
    assert!(validate_namespace("my-app_1").is_ok());
    assert_eq!(namespace_lock("example"), namespace_lock("example"));
    assert_ne!(namespace_lock("one"), namespace_lock("two"));
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn registration_survives_reopen_and_is_idempotent_per_namespace() {
    let first = Scenario::new();
    let second = Scenario::new();
    let original = visibility();
    assert_eq!(
        first
            .open()
            .register_models(&first.namespace, &original)
            .unwrap(),
        ModelRegistration {
            inserted: 1,
            unchanged: 0
        }
    );
    let mut store = first.open();
    assert_eq!(
        store.register_models(&first.namespace, &original).unwrap(),
        ModelRegistration {
            inserted: 0,
            unchanged: 1
        }
    );
    assert_eq!(
        store
            .registered_model(&first.namespace, "records", 1)
            .unwrap(),
        Some(original.models()[0].clone())
    );
    assert_eq!(
        store
            .registered_model(&second.namespace, "records", 1)
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .registered_model(&first.namespace, "records", 2)
            .unwrap(),
        None
    );

    let mut changed = original.models()[0].clone();
    changed.fields.insert(
        "newField".into(),
        Expression::Literal {
            value: cantongrid_core::value::Value::Text("private-definition".into()),
        },
    );
    let changed = catalog(&[changed]);
    assert_eq!(
        store.register_models(&first.namespace, &changed),
        Err(StoreError::ModelConflict)
    );
    assert_eq!(
        store
            .register_models(&second.namespace, &changed)
            .unwrap()
            .inserted,
        1
    );
    let mut next = changed.models()[0].clone();
    next.revision = u32::MAX;
    assert_eq!(
        store
            .register_models(&first.namespace, &catalog(&[next.clone()]))
            .unwrap()
            .inserted,
        1
    );
    assert_eq!(
        store
            .registered_model(&first.namespace, "records", u32::MAX)
            .unwrap(),
        Some(next)
    );
    assert_eq!(
        store
            .registered_model(&first.namespace, "records", 1)
            .unwrap(),
        Some(original.models()[0].clone())
    );
    assert!(!format!("{}", StoreError::ModelConflict).contains("private-definition"));
    let outcomia =
        Catalog::parse(include_str!("../../../../examples/models/outcomia.json")).unwrap();
    assert_eq!(
        store
            .register_models(&first.namespace, &outcomia)
            .unwrap()
            .inserted,
        1
    );
    assert_eq!(
        store
            .registered_model(&first.namespace, "markets", 1)
            .unwrap(),
        Some(outcomia.models()[0].clone())
    );
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn later_conflicts_roll_back_the_entire_catalog() {
    let scenario = Scenario::new();
    let original = visibility();
    let mut store = scenario.open();
    store
        .register_models(&scenario.namespace, &original)
        .unwrap();
    let mut new = original.models()[0].clone();
    new.name = "aNew".into(); // Sorted before the conflicting existing model.
    let mut conflict = original.models()[0].clone();
    conflict.fields.remove("note");
    let mixed = catalog(&[new, conflict]);
    assert_eq!(
        store.register_models(&scenario.namespace, &mixed),
        Err(StoreError::ModelConflict)
    );
    assert_eq!(
        store
            .registered_model(&scenario.namespace, "aNew", 1)
            .unwrap(),
        None
    );
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn registry_content_drift_is_rejected_instead_of_silently_repaired() {
    let scenario = Scenario::new();
    let original = visibility();
    let mut store = scenario.open();
    store
        .register_models(&scenario.namespace, &original)
        .unwrap();
    store
        .client
        .execute(
            "UPDATE cantongrid.model_revisions SET fingerprint=repeat('0',64) WHERE namespace=$1",
            &[&scenario.namespace],
        )
        .unwrap();
    assert_eq!(
        store.registered_model(&scenario.namespace, "records", 1),
        Err(StoreError::ModelCorrupt)
    );
    assert_eq!(
        store.register_models(&scenario.namespace, &original),
        Err(StoreError::ModelCorrupt)
    );
    let fingerprint: String = store
        .client
        .query_one(
            "SELECT fingerprint FROM cantongrid.model_revisions WHERE namespace=$1",
            &[&scenario.namespace],
        )
        .unwrap()
        .get(0);
    assert_eq!(fingerprint, "0".repeat(64));
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn namespace_registration_is_serialized_without_blocking_other_namespaces() {
    let first = Scenario::new();
    let second = Scenario::new();
    let mut blocker = first.open();
    let mut tx = blocker.client.transaction().unwrap();
    tx.execute(
        "SELECT pg_advisory_xact_lock($1)",
        &[&namespace_lock(&first.namespace)],
    )
    .unwrap();
    let mut store = first.open();
    assert_eq!(
        store.register_models(&first.namespace, &visibility()),
        Err(StoreError::Busy)
    );
    assert_eq!(
        store
            .register_models(&second.namespace, &visibility())
            .unwrap()
            .inserted,
        1
    );
    tx.rollback().unwrap();
    assert_eq!(
        store
            .register_models(&first.namespace, &visibility())
            .unwrap()
            .inserted,
        1
    );
}
