//! Explicit schema ownership/identity checks before any destination DDL.
use crate::StoreError;
use cantongrid_postgres::{
    connect_env,
    postgres::{Client, GenericClient},
};
use sha2::{Digest, Sha256};

const MIGRATION: &str = include_str!("../migrations/001_initial.sql");
// One database-local migration lock, separate from per-stream writer locks.
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
        check_schema(&mut tx)?;
    } else {
        tx.batch_execute(MIGRATION)?;
        tx.execute(
            "INSERT INTO cantongrid.schema_version VALUES (true, 1, $1)",
            &[&checksum()],
        )?;
    }
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
        check_schema(&mut client).unwrap();
    }
}
