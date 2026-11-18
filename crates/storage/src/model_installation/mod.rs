//! Atomic installation of compiler-generated schemas. No activation, copying or caller grants.
mod schema;

use crate::{Store, StoreError, model_registry};
use cantongrid_postgres::postgres::Transaction;
use serde::Serialize;
use sha2::{Digest, Sha256};

// Global because identical definitions in different namespaces share one physical table.
const INSTALL_LOCK: i64 = 0x43475249444d4f44;

/// Operator result contains identity only. A linked revision is not an active/live model.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ModelInstallation {
    pub table: String,
    pub fingerprint: String,
    pub created: bool,
    pub linked: bool,
}

impl Store {
    /// Install exactly one registered revision into the owned output schema. Repeating the
    /// operation verifies the real table before returning a no-op; it never adopts an unmanaged table.
    pub fn install_model(
        &mut self,
        namespace: &str,
        name: &str,
        revision: u32,
    ) -> Result<ModelInstallation, StoreError> {
        self.install_model_inner(namespace, name, revision, |_| Ok(()))
    }

    /// The private pre-commit hook lets tests inject an actual SQL abort after DDL/metadata writes.
    /// Production always supplies a no-op; it is not an executable-plugin surface.
    fn install_model_inner(
        &mut self,
        namespace: &str,
        name: &str,
        revision: u32,
        before_commit: impl FnOnce(&mut Transaction<'_>) -> Result<(), StoreError>,
    ) -> Result<ModelInstallation, StoreError> {
        model_registry::validate_key(namespace, name, revision)?;
        let mut tx = self.client.transaction()?;
        let acquired: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&INSTALL_LOCK])?
            .get(0);
        if !acquired {
            return Err(StoreError::Busy);
        }
        // Protect the registry row against direct SQL changes while compiling/installing it.
        if tx
            .query_opt(
                "SELECT 1 FROM cantongrid.model_revisions
             WHERE namespace=$1 AND name=$2 AND revision=$3 FOR SHARE",
                &[&namespace, &name, &i64::from(revision)],
            )?
            .is_none()
        {
            return Err(StoreError::ModelMissing);
        }
        let model = model_registry::load(&mut tx, namespace, name, revision)?
            .ok_or(StoreError::ModelMissing)?;
        let plan = model.sql_plan().map_err(|_| StoreError::ModelCorrupt)?;
        let columns = model.sql_columns().map_err(|_| StoreError::ModelCorrupt)?;
        let ddl_sha256: String = Sha256::digest(plan.create_sql.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let alias = tx.query_opt(
            "SELECT fingerprint FROM cantongrid.model_installations
             WHERE namespace=$1 AND name=$2 AND revision=$3 FOR SHARE",
            &[&namespace, &name, &i64::from(revision)],
        )?;
        if alias
            .as_ref()
            .is_some_and(|row| row.get::<_, String>(0) != plan.fingerprint)
        {
            return Err(StoreError::ModelSchema);
        }
        let physical = tx.query(
            "SELECT fingerprint, table_name, plan_version, ddl_sha256 FROM cantongrid.model_tables
             WHERE fingerprint=$1 OR table_name=$2 FOR SHARE",
            &[&plan.fingerprint, &plan.table],
        )?;
        if physical.len() > 1 {
            return Err(StoreError::ModelSchema);
        }
        let created = physical.is_empty();
        if let Some(row) = physical.first() {
            if row.get::<_, String>(0) != plan.fingerprint
                || row.get::<_, String>(1) != plan.table
                || row.get::<_, i16>(2) != 1
                || row.get::<_, String>(3) != ddl_sha256
            {
                return Err(StoreError::ModelSchema);
            }
        } else if alias.is_some() || schema::table_oid(&mut tx, &plan)?.is_some() {
            return Err(StoreError::ModelSchema);
        }
        schema::ensure_schema(&mut tx)?;
        if created {
            // DDL comes only from this compiler invocation, never from registry SQL text.
            tx.batch_execute(&plan.create_sql)?;
        }
        schema::verify_table(&mut tx, &plan, &columns)?;
        if created {
            tx.execute(
                "INSERT INTO cantongrid.model_tables(fingerprint, table_name, plan_version, ddl_sha256)
                 VALUES($1,$2,1,$3)", &[&plan.fingerprint, &plan.table, &ddl_sha256]
            )?;
        }
        let linked = alias.is_none();
        if linked {
            tx.execute(
                "INSERT INTO cantongrid.model_installations(namespace, name, revision, fingerprint)
                 VALUES($1,$2,$3,$4)",
                &[&namespace, &name, &i64::from(revision), &plan.fingerprint],
            )?;
        }
        before_commit(&mut tx)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(ModelInstallation {
            table: plan.table,
            fingerprint: plan.fingerprint,
            created,
            linked,
        })
    }
}

#[cfg(test)]
mod tests;
