//! PostgreSQL 14 catalog checks. Stored hashes identify a plan; they do not prove table shape.
use crate::StoreError;
use cantongrid_models::{SqlColumn, SqlPlan};
use cantongrid_postgres::postgres::{GenericClient, Transaction};

const RELATION: &str = include_str!("sql/relation.sql");
const COLUMNS: &str = include_str!("sql/columns.sql");
const CONSTRAINTS: &str = include_str!("sql/constraints.sql");
const INDEX: &str = include_str!("sql/index.sql");

/// Create the private output schema only when absent. Existing ownership/ACL drift is an error,
/// never permission to adopt or silently revoke another operator's grants.
pub(crate) fn ensure_schema(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let exists: bool = client
        .query_one(
            "SELECT to_regnamespace('cantongrid_models') IS NOT NULL",
            &[],
        )?
        .get(0);
    if !exists {
        client.batch_execute(
            "CREATE SCHEMA cantongrid_models AUTHORIZATION CURRENT_USER;
             REVOKE ALL ON SCHEMA cantongrid_models FROM PUBLIC;",
        )?;
    }
    check_schema(client)
}

/// Live processing must reject a missing schema, never create it during ingestion.
pub(crate) fn check_schema(client: &mut impl GenericClient) -> Result<(), StoreError> {
    let safe = client
        .query_opt(
            "SELECT nspowner = (SELECT oid FROM pg_roles WHERE rolname = current_user)
            AND NOT EXISTS (
                SELECT 1 FROM aclexplode(COALESCE(nspacl, acldefault('n', nspowner))) acl
                WHERE acl.grantee <> nspowner
            )
         FROM pg_namespace WHERE nspname = 'cantongrid_models'",
            &[],
        )?
        .is_some_and(|row| row.get::<_, bool>(0));
    if !safe {
        return Err(StoreError::ModelSchema);
    }
    Ok(())
}

/// Lookup by a fully qualified compiler-generated name; never execute stored/user-supplied SQL.
pub(crate) fn table_oid(
    client: &mut impl GenericClient,
    plan: &SqlPlan,
) -> Result<Option<u32>, StoreError> {
    Ok(client
        .query_one(
            "SELECT to_regclass($1)::oid",
            &[&format!("cantongrid_models.\"{}\"", plan.table)],
        )?
        .get(0))
}

/// Freeze the relation while checking it. This rare operator command blocks writes/schema changes
/// briefly; normal query/worker paths do not acquire this installation lock.
pub(crate) fn verify_table(
    tx: &mut Transaction<'_>,
    plan: &SqlPlan,
    columns: &[SqlColumn],
) -> Result<(), StoreError> {
    verify_locked(tx, plan, columns, "SHARE ROW EXCLUSIVE")
}

/// Row-exclusive locks block destructive DDL while allowing independent streams to write.
pub(crate) fn verify_writable(
    tx: &mut Transaction<'_>,
    plan: &SqlPlan,
    columns: &[SqlColumn],
) -> Result<(), StoreError> {
    verify_locked(tx, plan, columns, "ROW EXCLUSIVE")
}

/// Lock modes come only from the two fixed callers above, never from configuration.
fn verify_locked(
    tx: &mut Transaction<'_>,
    plan: &SqlPlan,
    columns: &[SqlColumn],
    mode: &str,
) -> Result<(), StoreError> {
    let oid = table_oid(tx, plan)?.ok_or(StoreError::ModelSchema)?;
    // Reject views/foreign/partitioned relations before LOCK TABLE can follow their dependencies.
    let ordinary: bool = tx.query_one(
        "SELECT relkind = 'r' AND relowner = (SELECT oid FROM pg_roles WHERE rolname=current_user)
         FROM pg_class WHERE oid=$1", &[&oid]
    )?.get(0);
    if !ordinary {
        return Err(StoreError::ModelSchema);
    }
    // Only validated hash-derived names reach this interpolation. ONLY prevents following
    // unexpected inheritance children; the catalog checks below reject inheritance outright.
    tx.batch_execute(&format!(
        "LOCK TABLE ONLY cantongrid_models.\"{}\" IN {mode} MODE",
        plan.table
    ))?;
    if table_oid(tx, plan)? != Some(oid) || !tx.query_one(RELATION, &[&oid])?.get::<_, bool>(0) {
        return Err(StoreError::ModelSchema);
    }
    let actual = tx.query(COLUMNS, &[&oid])?;
    if actual.len() != columns.len() {
        return Err(StoreError::ModelSchema);
    }
    for (row, expected) in actual.iter().zip(columns) {
        if !row.get::<_, bool>(3)
            || row.get::<_, String>(0) != expected.name
            || row.get::<_, String>(1) != expected.sql_type
            || row.get::<_, bool>(2) == expected.nullable
        {
            return Err(StoreError::ModelSchema);
        }
    }
    let constraints = tx.query(CONSTRAINTS, &[&oid])?;
    if constraints.len() != 2 {
        return Err(StoreError::ModelSchema);
    }
    let mut primary = None;
    let mut foreign = false;
    for row in constraints {
        if !row.get::<_, bool>(3) || row.get::<_, Option<Vec<i16>>>(1) != Some(vec![1, 2]) {
            return Err(StoreError::ModelSchema);
        }
        match row.get::<_, String>(0).as_str() {
            "p" if primary.is_none() => primary = Some(row.get::<_, u32>(2)),
            "f" if !foreign => foreign = true,
            _ => return Err(StoreError::ModelSchema),
        }
    }
    let primary = primary.filter(|_| foreign).ok_or(StoreError::ModelSchema)?;
    if !tx.query_one(INDEX, &[&oid, &primary])?.get::<_, bool>(0) {
        return Err(StoreError::ModelSchema);
    }
    Ok(())
}
