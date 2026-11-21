//! Compile stored declarations again and verify their private physical tables before use.
use crate::{StoreError, model_installation::schema, model_registry};
use cantongrid_models::{Model, SqlPlan};
use cantongrid_postgres::postgres::Transaction;
use sha2::{Digest, Sha256};

/// A declaration whose registry, installation metadata and real schema agree in this transaction.
/// The table lock lasts until commit; this value never authenticates source or API access.
pub(crate) struct Installed {
    pub model: Model,
    pub sql: SqlPlan,
}

/// Only compiler-derived identifiers reach DML. Stored SQL and table names are never executed.
pub(crate) fn load(
    tx: &mut Transaction<'_>,
    namespace: &str,
    name: &str,
    revision: u32,
) -> Result<Installed, StoreError> {
    load_checked(tx, namespace, name, revision, false)
}

/// Query snapshots cannot acquire row locks; immutable metadata and relation locks protect reads.
pub(super) fn read(
    tx: &mut Transaction<'_>,
    namespace: &str,
    name: &str,
    revision: u32,
) -> Result<Installed, StoreError> {
    load_checked(tx, namespace, name, revision, true)
}

/// Recompile stored definitions and verify relation identity/shape under the appropriate read or write lock.
fn load_checked(
    tx: &mut Transaction<'_>,
    namespace: &str,
    name: &str,
    revision: u32,
    read_only: bool,
) -> Result<Installed, StoreError> {
    model_registry::validate_key(namespace, name, revision)?;
    let metadata = tx
        .query_opt(
            &format!(
                "SELECT i.fingerprint,t.table_name,t.plan_version,t.ddl_sha256
         FROM cantongrid.model_installations i
         JOIN cantongrid.model_revisions r USING(namespace,name,revision)
         JOIN cantongrid.model_tables t ON t.fingerprint=i.fingerprint
         WHERE i.namespace=$1 AND i.name=$2 AND i.revision=$3 {}",
                if read_only { "" } else { "FOR SHARE OF i,r,t" }
            ),
            &[&namespace, &name, &i64::from(revision)],
        )?
        .ok_or(StoreError::ModelMissing)?;
    let model =
        model_registry::load(tx, namespace, name, revision)?.ok_or(StoreError::ModelMissing)?;
    let sql = model.sql_plan().map_err(|_| StoreError::ModelCorrupt)?;
    let ddl_sha256: String = Sha256::digest(sql.create_sql.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if metadata.get::<_, String>(0) != sql.fingerprint
        || metadata.get::<_, String>(1) != sql.table
        || metadata.get::<_, i16>(2) != 1
        || metadata.get::<_, String>(3) != ddl_sha256
    {
        return Err(StoreError::ModelSchema);
    }
    schema::check_schema(tx)?;
    let columns = model.sql_columns().map_err(|_| StoreError::ModelCorrupt)?;
    if read_only {
        schema::verify_readable(tx, &sql, &columns)?;
    } else {
        schema::verify_writable(tx, &sql, &columns)?;
    }
    Ok(Installed { model, sql })
}

impl Installed {
    /// Fully qualified, quoted identifier generated from the validated model fingerprint.
    pub fn relation(&self) -> String {
        format!("cantongrid_models.\"{}\"", self.sql.table)
    }
}
