//! Operator-only definitions, independent from source checkpoints and caller permissions.
use crate::{Store, StoreError};
use cantongrid_models::{Catalog, Model};
use cantongrid_postgres::postgres::{GenericClient, Row};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// A registration never installs projection tables or activates a model. Counts contain no literals.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ModelRegistration {
    pub inserted: u32,
    pub unchanged: u32,
}

impl Store {
    /// Register one bounded catalog atomically. A namespace identifies an operator's project,
    /// not a source or permission scope. There is intentionally no update/delete API.
    pub fn register_models(
        &mut self,
        namespace: &str,
        catalog: &Catalog,
    ) -> Result<ModelRegistration, StoreError> {
        validate_namespace(namespace)?;
        let mut tx = self.client.transaction()?;
        let lock = namespace_lock(namespace);
        let acquired: bool = tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&lock])?
            .get(0);
        if !acquired {
            return Err(StoreError::Busy);
        }
        let mut result = ModelRegistration {
            inserted: 0,
            unchanged: 0,
        };
        for model in catalog.models() {
            if let Some(existing) = load(&mut tx, namespace, &model.name, model.revision)? {
                if existing != *model {
                    return Err(StoreError::ModelConflict);
                }
                result.unchanged += 1;
                continue;
            }
            let fingerprint = model
                .fingerprint()
                .map_err(|_| StoreError::ModelDefinition)?;
            let definition =
                serde_json::to_value(model).map_err(|_| StoreError::ModelDefinition)?;
            tx.execute(
                "INSERT INTO cantongrid.model_revisions
                    (namespace, name, revision, definition_version, fingerprint, definition)
                 VALUES ($1, $2, $3, 1, $4, $5)",
                &[
                    &namespace,
                    &model.name,
                    &i64::from(model.revision),
                    &fingerprint,
                    &definition,
                ],
            )?;
            result.inserted += 1;
        }
        // All definitions roll back on any conflict. An uncertain COMMIT can be retried safely.
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(result)
    }

    /// Load one exact revision for a trusted operator. Revalidate schema and fingerprint before
    /// returning it; model definitions never grant permission to query canonical contract data.
    pub fn registered_model(
        &mut self,
        namespace: &str,
        name: &str,
        revision: u32,
    ) -> Result<Option<Model>, StoreError> {
        validate_key(namespace, name, revision)?;
        load(&mut self.client, namespace, name, revision)
    }
}

/// Shared identity validation for registry reads and operator installation commands.
pub(crate) fn validate_key(namespace: &str, name: &str, revision: u32) -> Result<(), StoreError> {
    validate_namespace(namespace)?;
    if name.is_empty()
        || name.len() > 48
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || revision == 0
    {
        return Err(StoreError::ModelDefinition);
    }
    Ok(())
}

/// Always check persisted content, including on an idempotent retry; do not silently repair drift.
pub(crate) fn load(
    client: &mut impl GenericClient,
    namespace: &str,
    name: &str,
    revision: u32,
) -> Result<Option<Model>, StoreError> {
    let row = client.query_opt(
        "SELECT definition_version, fingerprint, definition
         FROM cantongrid.model_revisions
         WHERE namespace = $1 AND name = $2 AND revision = $3",
        &[&namespace, &name, &i64::from(revision)],
    )?;
    row.map(|row| decode(&row, name, revision)).transpose()
}

/// Reconstruct through the public strict compiler rather than trusting a stored Rust/SQL plan.
fn decode(row: &Row, name: &str, revision: u32) -> Result<Model, StoreError> {
    if row.get::<_, i16>(0) != 1 {
        return Err(StoreError::ModelCorrupt);
    }
    let definition: serde_json::Value = row.get(2);
    let document = serde_json::json!({"version":1,"models":[definition]}).to_string();
    let catalog = Catalog::parse(&document).map_err(|_| StoreError::ModelCorrupt)?;
    let model = &catalog.models()[0];
    if model.name != name
        || model.revision != revision
        || model.fingerprint().map_err(|_| StoreError::ModelCorrupt)? != row.get::<_, String>(1)
    {
        return Err(StoreError::ModelCorrupt);
    }
    Ok(model.clone())
}

/// Bound operator labels before database access. Their meaning is local to this owned store.
fn validate_namespace(namespace: &str) -> Result<(), StoreError> {
    if namespace.is_empty()
        || namespace.len() > 64
        || !namespace.as_bytes()[0].is_ascii_alphabetic()
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(StoreError::ModelDefinition);
    }
    Ok(())
}

/// Serialize a namespace's registrations. Hash collisions only cause temporary contention;
/// the primary key and full namespace still distinguish all stored revisions.
fn namespace_lock(namespace: &str) -> i64 {
    let input = serde_json::json!(["cantongrid-model-registry-v1", namespace]).to_string();
    let digest = Sha256::digest(input.as_bytes());
    i64::from_be_bytes(digest[..8].try_into().expect("SHA-256 has 32 bytes"))
}

#[cfg(test)]
mod tests;
