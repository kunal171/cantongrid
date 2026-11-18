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
        "SELECT EXISTS (SELECT 1 FROM pg_class WHERE oid=to_regclass('cantongrid.model_revisions') AND relkind='r' AND relowner=(SELECT oid FROM pg_roles WHERE rolname=current_user))",
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
    if !versions.contains(&3) {
        client.batch_execute(MODELS)?;
        client.execute(
            "INSERT INTO cantongrid.runtime_migrations VALUES(3,$1)",
            &[&models_checksum()],
        )?;
    }
    check_schema(client)
}

/// Reads require all migrations. Initialization may additionally accept the intact 002 baseline.
fn check_runtime_versions(
    client: &mut impl GenericClient,
    require_models: bool,
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
        "SELECT version,migration_sha256 FROM cantongrid.runtime_migrations",
        &[],
    )?;
    let mut versions = Vec::new();
    for row in rows {
        let version: i32 = row.get(0);
        let expected = match version {
            2 => progress_checksum(),
            3 => models_checksum(),
            _ => return Err(StoreError::Schema),
        };
        if row.get::<_, String>(1) != expected {
            return Err(StoreError::Schema);
        }
        versions.push(version);
    }
    if !versions.contains(&2) || (require_models && !versions.contains(&3)) {
        return Err(StoreError::Schema);
    }
    Ok(versions)
}

fn models_checksum() -> String {
    Sha256::digest(MODELS.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn progress_checksum() -> String {
    Sha256::digest(PROGRESS.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn check_base(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let owned: bool = client.query_one("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname='cantongrid' AND nspowner=(SELECT oid FROM pg_roles WHERE rolname=current_user)) AND to_regclass('cantongrid.schema_version') IS NOT NULL", &[])?.get(0);
    if !owned {
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
        tx.batch_execute("DROP TABLE cantongrid.model_revisions; DELETE FROM cantongrid.runtime_migrations WHERE version=3").unwrap();
        assert_eq!(check_schema(&mut tx), Err(StoreError::Schema));
        upgrade_models(&mut tx).unwrap();
        upgrade_models(&mut tx).unwrap(); // Reinitialization never rewrites a recorded checksum.
        assert_eq!(
            tx.query_one("SELECT count(*) FROM cantongrid.runtime_migrations", &[])
                .unwrap()
                .get::<_, i64>(0),
            2
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
}
