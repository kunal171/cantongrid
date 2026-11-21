//! Explicit schema ownership/identity checks before any destination DDL.
use crate::StoreError;
use cantongrid_postgres::{
    connect_env,
    postgres::{Client, GenericClient},
};
use sha2::{Digest, Sha256};

const MIGRATION: &str = include_str!("../migrations/001_initial.sql");
// One database-local migration lock, separate from per-stream writer locks.
const PROGRESS: &str = include_str!("../migrations/002_progress.sql");
// The registry allows 128 KiB of JSONB text for a compiler input capped at 64 KiB,
// accounting for PostgreSQL's extra JSON formatting whitespace.
const MODELS: &str = include_str!("../migrations/003_models.sql");
const INSTALLATIONS: &str = include_str!("../migrations/004_model_installations.sql");
const BINDINGS: &str = include_str!("../migrations/005_model_bindings.sql");
const REBUILDS: &str = include_str!("../migrations/006_model_rebuilds.sql");
const UPGRADES: &str = include_str!("../migrations/007_selection_upgrades.sql");
const UPGRADE_JOBS: &str = include_str!("../migrations/008_upgrade_jobs.sql");
const EXERCISES: &str = include_str!("../migrations/009_exercise_receipts.sql");
const LINEAGE: &str = include_str!("../migrations/010_direct_lineage.sql");
const MIGRATION_LOCK: i64 = 0x43475249444d4947;

/// Initialize only a separately owned destination; repeat calls verify the immutable migration.
pub fn initialize_env(variable: &str) -> Result<(), StoreError> {
    let mut client = connect_env(variable)?;
    initialize(&mut client)
}

/// Use an explicit transaction so failed DDL never leaves an apparently usable partial schema.
fn initialize(client: &mut Client) -> Result<(), StoreError> {
    let mut tx = client.transaction()?;
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])?;
    check_destination(&mut tx)?;
    let exists: bool = tx
        .query_one("SELECT to_regnamespace('cantongrid') IS NOT NULL", &[])?
        .get(0);
    if exists {
        check_base(&mut tx)?;
    } else {
        tx.batch_execute(MIGRATION)?;
        tx.execute(
            "INSERT INTO cantongrid.schema_version VALUES (true, 1, $1)",
            &[&checksum()],
        )?;
    }
    let progress: bool = tx
        .query_one(
            "SELECT to_regclass('cantongrid.runtime_migrations') IS NOT NULL",
            &[],
        )?
        .get(0);
    if !progress {
        tx.batch_execute(PROGRESS)?;
        tx.execute(
            "INSERT INTO cantongrid.runtime_migrations VALUES(2,$1)",
            &[&progress_checksum()],
        )?;
    }
    upgrade_models(&mut tx)?;
    tx.commit().map_err(|_| StoreError::CommitUnknown)
}

/// Ownership and source-role checks apply on every reopened destination connection.
pub(crate) fn check_destination(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let safe: bool = client.query_one("SELECT d.datdba = r.oid AND NOT (r.rolsuper OR r.rolcreatedb OR r.rolcreaterole OR r.rolbypassrls OR r.rolreplication) AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member=r.oid) AND d.datname NOT IN ('postgres','template0','template1') AND to_regprocedure('public.active(text,bigint)') IS NULL FROM pg_database d CROSS JOIN pg_roles r WHERE d.datname=current_database() AND r.rolname=current_user", &[])?.get(0);
    if !safe {
        return Err(StoreError::UnsafeDestination);
    }
    Ok(())
}

/// Never silently adopt a preexisting schema or rewrite an applied migration checksum.
pub(crate) fn check_schema(client: &mut impl GenericClient) -> Result<(), StoreError> {
    check_base(client)?;
    check_runtime_versions(client, true)?;
    let owned: bool = client.query_one(
        "SELECT count(*) = 23 AND bool_and(relkind='r' AND relowner=(SELECT oid FROM pg_roles WHERE rolname=current_user))
         FROM pg_class WHERE oid IN (to_regclass('cantongrid.schema_version'),
             to_regclass('cantongrid.streams'),to_regclass('cantongrid.batches'),
             to_regclass('cantongrid.contracts'),to_regclass('cantongrid.events'),
             to_regclass('cantongrid.outbox'),to_regclass('cantongrid.runtime_migrations'),
             to_regclass('cantongrid.model_revisions'),
             to_regclass('cantongrid.model_tables'), to_regclass('cantongrid.model_installations'), to_regclass('cantongrid.model_bindings'),
             to_regclass('cantongrid.model_rebuilds'),to_regclass('cantongrid.model_rebuild_dirty'),to_regclass('cantongrid.selection_upgrades'),to_regclass('cantongrid.selection_upgrade_jobs'),to_regclass('cantongrid.selection_upgrade_pages'),to_regclass('cantongrid.exercise_pages'),to_regclass('cantongrid.exercise_events'),to_regclass('cantongrid.exercise_outbox'),to_regclass('cantongrid.lineage_pages'),to_regclass('cantongrid.creation_events'),to_regclass('cantongrid.lineage_edges'),to_regclass('cantongrid.lineage_outbox'))",
        &[],
    )?.get(0);
    if !owned {
        return Err(StoreError::Schema);
    }
    Ok(())
}

/// Accept the previous supported schema only during explicit initialization.
/// Validate existing checksums first, so an unknown migration is never adopted.
fn upgrade_models(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let versions = check_runtime_versions(client, false)?;
    for (version, sql) in [
        (3, MODELS),
        (4, INSTALLATIONS),
        (5, BINDINGS),
        (6, REBUILDS),
        (7, UPGRADES),
        (8, UPGRADE_JOBS),
        (9, EXERCISES),
        (10, LINEAGE),
    ] {
        if !versions.contains(&version) {
            client.batch_execute(sql)?;
            client.execute(
                "INSERT INTO cantongrid.runtime_migrations VALUES($1,$2)",
                &[
                    &version,
                    &runtime_checksum(version).ok_or(StoreError::Schema)?,
                ],
            )?;
        }
    }
    check_schema(client)
}

/// Reads require the full set. Initialization accepts only an intact ordered prefix;
/// for example, [2, 4] is corruption, not permission to recreate migration 003.
fn check_runtime_versions(
    client: &mut impl GenericClient,
    require_current: bool,
) -> Result<Vec<i32>, StoreError> {
    let exists: bool = client
        .query_one(
            "SELECT to_regclass('cantongrid.runtime_migrations') IS NOT NULL",
            &[],
        )?
        .get(0);
    if !exists {
        return Err(StoreError::Schema);
    }
    let rows = client.query(
        "SELECT version,migration_sha256 FROM cantongrid.runtime_migrations ORDER BY version",
        &[],
    )?;
    let mut versions = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        let version: i32 = row.get(0);
        if [2, 3, 4, 5, 6, 7, 8, 9, 10].get(index) != Some(&version) {
            return Err(StoreError::Schema);
        }
        let expected = runtime_checksum(version).ok_or(StoreError::Schema)?;
        if row.get::<_, String>(1) != expected {
            return Err(StoreError::Schema);
        }
        versions.push(version);
    }
    if versions.is_empty() || (require_current && versions.len() != 9) {
        return Err(StoreError::Schema);
    }
    Ok(versions)
}

/// Checksum the exact applied SQL; even comments in an applied migration stay immutable.
fn runtime_checksum(version: i32) -> Option<String> {
    let sql = match version {
        2 => PROGRESS,
        3 => MODELS,
        4 => INSTALLATIONS,
        5 => BINDINGS,
        6 => REBUILDS,
        7 => UPGRADES,
        8 => UPGRADE_JOBS,
        9 => EXERCISES,
        10 => LINEAGE,
        _ => return None,
    };
    Some(
        Sha256::digest(sql.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// Compare immutable migration bytes with recorded history; never recompute an applied SQL definition.
fn progress_checksum() -> String {
    Sha256::digest(PROGRESS.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Verify owned schema identity, private object grants and the original migration before opening state.
fn check_base(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let owned: bool = client.query_one("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname='cantongrid' AND nspowner=(SELECT oid FROM pg_roles WHERE rolname=current_user)) AND to_regclass('cantongrid.schema_version') IS NOT NULL", &[])?.get(0);
    if !owned {
        return Err(StoreError::Schema);
    }
    // A revoked schema grant alone does not prove that tables, sequences or individual
    // columns remain private. Reject drift instead of silently changing an operator's ACLs.
    let private: bool = client
        .query_one(include_str!("sql/private_destination.sql"), &[])?
        .get(0);
    if !private {
        return Err(StoreError::Schema);
    }
    let rows = client.query(
        "SELECT version, migration_sha256 FROM cantongrid.schema_version",
        &[],
    )?;
    if rows.len() != 1 || rows[0].get::<_, i32>(0) != 1 || rows[0].get::<_, String>(1) != checksum()
    {
        return Err(StoreError::Schema);
    }
    Ok(())
}

/// SHA-256 is for detecting migration drift, not an authorization capability.
fn checksum() -> String {
    Sha256::digest(MIGRATION.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Privilege drift must fail even when migration checksums and table ownership still match.
    /// Newer tables must be removed with their migration entries when reconstructing an old
    /// schema inside a rollback-only test. Leaving newer entries is intentionally rejected.
    fn drop_upgrade_schema(tx: &mut cantongrid_postgres::postgres::Transaction<'_>) {
        tx.batch_execute("DROP TABLE cantongrid.lineage_outbox,cantongrid.lineage_edges,cantongrid.creation_events,cantongrid.lineage_pages; DELETE FROM cantongrid.runtime_migrations WHERE version=10; DROP TABLE cantongrid.exercise_outbox,cantongrid.exercise_events,cantongrid.exercise_pages; DELETE FROM cantongrid.runtime_migrations WHERE version=9; DROP TABLE cantongrid.selection_upgrade_pages,cantongrid.selection_upgrade_jobs,cantongrid.selection_upgrades; DELETE FROM cantongrid.runtime_migrations WHERE version IN (7,8)").unwrap();
    }

    #[test]
    #[ignore = "requires owned PostgreSQL; run serially"]
    fn canonical_schema_tables_columns_and_sequences_reject_nonowner_grants() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        for grant in [
            "GRANT USAGE ON SCHEMA cantongrid TO PUBLIC",
            "GRANT SELECT ON cantongrid.contracts TO PUBLIC",
            "GRANT SELECT ON cantongrid.exercise_events TO PUBLIC",
            "GRANT SELECT ON cantongrid.lineage_edges TO PUBLIC",
            "GRANT SELECT(envelope) ON cantongrid.exercise_pages TO PUBLIC",
            "GRANT SELECT(payload) ON cantongrid.contracts TO PUBLIC",
            "GRANT SELECT(definition) ON cantongrid.model_revisions TO PUBLIC",
            "GRANT SELECT(contract_id) ON cantongrid.model_rebuild_dirty TO PUBLIC",
            "GRANT SELECT ON cantongrid.runtime_migrations TO PUBLIC",
            "GRANT SELECT ON cantongrid.selection_upgrade_pages TO PUBLIC",
            "GRANT SELECT(manifest) ON cantongrid.selection_upgrade_jobs TO PUBLIC",
            "GRANT USAGE ON SEQUENCE cantongrid.outbox_delivery_id_seq TO PUBLIC",
        ] {
            let mut tx = client.transaction().unwrap();
            tx.batch_execute(grant).unwrap();
            assert_eq!(check_schema(&mut tx), Err(StoreError::Schema), "{grant}");
            tx.rollback().unwrap();
        }
        check_schema(&mut client).unwrap();
    }

    #[test]
    #[ignore = "requires the owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
    fn initialization_is_idempotent_and_migration_drift_is_rejected() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute(
            "UPDATE cantongrid.schema_version SET migration_sha256='synthetic-drift'",
            &[],
        )
        .unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute(
            "UPDATE cantongrid.runtime_migrations SET migration_sha256='synthetic-progress-drift'",
            &[],
        )
        .unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        check_schema(&mut client).unwrap();
    }

    #[test]
    #[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
    fn model_registry_upgrade_is_explicit_and_preserves_previous_migrations() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        let fixture: bool = client
            .query_one(
                "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(
            fixture,
            "schema controls require the disposable destination"
        );
        // Restore a 002 schema inside a rollback-only transaction; real rows survive this control.
        let mut tx = client.transaction().unwrap();
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
            .unwrap();
        drop_upgrade_schema(&mut tx);
        tx.batch_execute("DROP TABLE cantongrid.model_rebuild_dirty,cantongrid.model_rebuilds; DROP TABLE cantongrid.model_bindings, cantongrid.model_installations, cantongrid.model_tables, cantongrid.model_revisions; DELETE FROM cantongrid.runtime_migrations WHERE version IN (3,4,5,6)").unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        upgrade_models(&mut tx).unwrap();
        upgrade_models(&mut tx).unwrap(); // Reinitialization never rewrites a recorded checksum.
        assert_eq!(
            tx.query_one("SELECT count(*) FROM cantongrid.runtime_migrations", &[])
                .unwrap()
                .get::<_, i64>(0),
            9
        );
        tx.execute(
            "INSERT INTO cantongrid.runtime_migrations VALUES(99,'synthetic-unknown-version')",
            &[],
        )
        .unwrap();
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.execute(
            "DELETE FROM cantongrid.runtime_migrations WHERE version=99",
            &[],
        )
        .unwrap();
        tx.execute("UPDATE cantongrid.runtime_migrations SET migration_sha256='synthetic-model-drift' WHERE version=3", &[]).unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        check_schema(&mut client).unwrap();
    }

    #[test]
    #[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
    fn installation_upgrade_preserves_registry_and_rejects_noncontiguous_history() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        let fixture: bool = client
            .query_one(
                "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(
            fixture,
            "upgrade control requires the disposable owned destination"
        );
        let mut tx = client.transaction().unwrap();
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
            .unwrap();
        let count: i64 = tx
            .query_one("SELECT count(*) FROM cantongrid.model_revisions", &[])
            .unwrap()
            .get(0);
        drop_upgrade_schema(&mut tx);
        tx.batch_execute("DROP TABLE cantongrid.model_rebuild_dirty,cantongrid.model_rebuilds; DROP TABLE cantongrid.model_bindings, cantongrid.model_installations, cantongrid.model_tables; DELETE FROM cantongrid.runtime_migrations WHERE version IN (4,5,6)").unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        upgrade_models(&mut tx).unwrap();
        assert_eq!(
            tx.query_one("SELECT count(*) FROM cantongrid.model_revisions", &[])
                .unwrap()
                .get::<_, i64>(0),
            count
        );
        tx.execute(
            "DELETE FROM cantongrid.runtime_migrations WHERE version=3",
            &[],
        )
        .unwrap();
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute("UPDATE cantongrid.runtime_migrations SET migration_sha256='synthetic-installation-drift' WHERE version IN (4,5,6)", &[]).unwrap();
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        check_schema(&mut client).unwrap();
    }
    #[test]
    #[ignore = "requires the disposable owned PostgreSQL fixture; run schema tests serially"]
    fn binding_upgrade_preserves_installed_tables_and_detects_checksum_drift() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
            .unwrap();
        let installed: i64 = tx
            .query_one("SELECT count(*) FROM cantongrid.model_tables", &[])
            .unwrap()
            .get(0);
        drop_upgrade_schema(&mut tx);
        tx.batch_execute("DROP TABLE cantongrid.model_rebuild_dirty,cantongrid.model_rebuilds; DROP TABLE cantongrid.model_bindings; DELETE FROM cantongrid.runtime_migrations WHERE version IN (5,6)").unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        upgrade_models(&mut tx).unwrap();
        upgrade_models(&mut tx).unwrap();
        assert_eq!(
            tx.query_one("SELECT count(*) FROM cantongrid.model_tables", &[])
                .unwrap()
                .get::<_, i64>(0),
            installed
        );
        tx.execute("UPDATE cantongrid.runtime_migrations SET migration_sha256='synthetic-binding-drift' WHERE version IN (5,6)", &[]).unwrap();
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        check_schema(&mut client).unwrap();
    }
    #[test]
    #[ignore = "requires disposable owned PostgreSQL; run serially"]
    fn rebuild_upgrade_is_explicit_and_rejects_checksum_and_public_access_drift() {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut client = connect_env("CANTONGRID_DESTINATION_URL").unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
            .unwrap();
        drop_upgrade_schema(&mut tx);
        tx.batch_execute("DROP TABLE cantongrid.model_rebuild_dirty,cantongrid.model_rebuilds; DELETE FROM cantongrid.runtime_migrations WHERE version=6").unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        upgrade_models(&mut tx).unwrap();
        tx.batch_execute("GRANT SELECT ON cantongrid.model_rebuild_dirty TO PUBLIC")
            .unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        tx.batch_execute("REVOKE SELECT ON cantongrid.model_rebuild_dirty FROM PUBLIC")
            .unwrap();
        tx.execute(
            "UPDATE cantongrid.runtime_migrations SET migration_sha256='drift' WHERE version=6",
            &[],
        )
        .unwrap();
        assert_eq!(upgrade_models(&mut tx), Err(StoreError::Schema));
        tx.rollback().unwrap();
        check_schema(&mut client).unwrap();
    }
}
