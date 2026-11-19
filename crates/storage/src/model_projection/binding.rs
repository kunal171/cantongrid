//! Bounded atomic backfill and compare-and-replace activation under the canonical source lock.
use super::{Budget, MAX_BINDINGS, MAX_WORK_BYTES, apply, authorized, installed, load_bound, lock};
use crate::{Store, StoreError, journal::stream_key, model_registry};
use cantongrid_core::{authorization::SourceLease, config::Plan, value::Value};
use cantongrid_postgres::postgres::Transaction;
use serde::Serialize;

const MAX_BACKFILL_ROWS: i64 = 128;

/// Operator acknowledgement only: counts and the destination batch revision, never payloads.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ModelBinding {
    pub changed: bool,
    pub input_rows: u32,
    pub projected_rows: u32,
    pub source_revision: String,
}

impl Store {
    /// Bind a previously installed revision after backfilling the currently known active prefix.
    /// Replacements require the expected old revision; failure leaves the old binding and rows intact.
    /// Larger data sets need a staged rebuild, which this atomic slice deliberately rejects.
    pub fn bind_model(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        namespace: &str,
        name: &str,
        revision: u32,
        expected_previous: Option<u32>,
    ) -> Result<ModelBinding, StoreError> {
        self.bind_model_checked(
            plan,
            lease,
            namespace,
            name,
            revision,
            expected_previous,
            |_| Ok(()),
        )
    }

    /// A private fault hook exercises rollback and lease revocation after all backfill writes.
    #[allow(clippy::too_many_arguments)]
    fn bind_model_checked(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        namespace: &str,
        name: &str,
        revision: u32,
        expected_previous: Option<u32>,
        before_commit: impl FnOnce(&mut Transaction<'_>) -> Result<(), StoreError>,
    ) -> Result<ModelBinding, StoreError> {
        model_registry::validate_key(namespace, name, revision)?;
        if !authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        if !plan.is_live()
            || !plan.source_binding().is_some_and(|b| {
                b.len() == 64
                    && b.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
        {
            return Err(StoreError::ModelBinding);
        }
        // READ COMMITTED is intentional: reads after acquiring the lock must see a preceding
        // worker's commit, even if that commit happened during the lock-acquisition statement.
        let mut tx = self.client.transaction()?;
        lock(&mut tx, plan)?;
        let source_revision = crate::live::prefix(&mut tx, plan)?
            .ok_or(StoreError::ModelBinding)?
            .0;
        let key = stream_key(plan).0;
        let old = tx.query_opt(
            "SELECT revision,fingerprint FROM cantongrid.model_bindings
             WHERE stream_key=$1 AND namespace=$2 AND name=$3",
            &[&key, &namespace, &name],
        )?;
        let bound = load_bound(&mut tx, plan, source_revision)?;
        let target = installed::load(&mut tx, namespace, name, revision)?;
        if super::rebuild::reserved(&mut tx, &key, namespace, name, &target.sql.fingerprint)? {
            return Err(StoreError::ModelRebuild);
        }
        if !plan.selects(&target.model.template) {
            return Err(StoreError::ModelBinding);
        }
        if old
            .as_ref()
            .is_some_and(|r| r.get::<_, i64>(0) == i64::from(revision))
        {
            // An uncertain COMMIT may already have promoted this revision. Verify it and return
            // safely without requiring the former revision or repeating an expensive backfill.
            if !authorized(plan, lease) {
                return Err(StoreError::Authorization);
            }
            tx.commit().map_err(|_| StoreError::CommitUnknown)?;
            return Ok(ModelBinding {
                changed: false,
                input_rows: 0,
                projected_rows: 0,
                source_revision: source_revision.to_string(),
            });
        }
        if old.as_ref().map(|r| r.get::<_, i64>(0)) != expected_previous.map(i64::from) {
            return Err(StoreError::ModelBinding);
        }
        if old.is_none() {
            let count: i64 = tx
                .query_one(
                    "SELECT count(*) FROM cantongrid.model_bindings WHERE stream_key=$1",
                    &[&key],
                )?
                .get(0);
            if count >= MAX_BINDINGS as i64 {
                return Err(StoreError::Limit);
            }
        }
        let template =
            serde_json::to_value(&target.model.template).map_err(|_| StoreError::ModelCorrupt)?;
        // Count a limited prefix and its stored bytes before loading private payloads into memory.
        let size = tx.query_one(
            "SELECT count(*),COALESCE(sum(octet_length(payload::text)),0)::bigint FROM
             (SELECT payload FROM cantongrid.contracts WHERE stream_key=$1 AND active AND template=$2
              ORDER BY contract_id LIMIT 129) bounded", &[&key, &template]
        )?;
        if size.get::<_, i64>(0) > MAX_BACKFILL_ROWS
            || size.get::<_, i64>(1) > MAX_WORK_BYTES as i64
        {
            return Err(StoreError::Limit);
        }
        clear(&mut tx, &target, &key)?;
        let rows = tx.query(
            "SELECT contract_id,payload,witnesses FROM cantongrid.contracts
             WHERE stream_key=$1 AND active AND template=$2 ORDER BY contract_id LIMIT 129",
            &[&key, &template],
        )?;
        let input_rows = u32::try_from(rows.len()).map_err(|_| StoreError::Limit)?;
        let mut budget = Budget::default();
        for row in rows {
            let witnesses: Vec<String> = row.get(2);
            if !witnesses
                .iter()
                .any(|p| plan.source_parties().any(|e| e.as_str() == p))
            {
                return Err(StoreError::Authorization);
            }
            let Value::RawJson(payload) =
                serde_json::from_value(row.get(1)).map_err(|_| StoreError::ModelProjection)?
            else {
                return Err(StoreError::ModelProjection);
            };
            apply(
                &mut tx,
                std::slice::from_ref(&target),
                super::Input {
                    template: &target.model.template,
                    stream: &key,
                    contract: row.get(0),
                    payload: &payload,
                    archive: false,
                },
                &mut budget,
            )?;
        }
        let projected_rows: i64 = tx
            .query_one(
                &format!(
                    "SELECT count(*) FROM {} WHERE \"_cg_stream_key\"=$1",
                    target.relation()
                ),
                &[&key],
            )?
            .get(0);
        tx.execute(
            "INSERT INTO cantongrid.model_bindings(stream_key,namespace,name,revision,fingerprint,bound_at_revision)
             VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(stream_key,namespace,name) DO UPDATE SET
             revision=EXCLUDED.revision,fingerprint=EXCLUDED.fingerprint,
             bound_at_revision=EXCLUDED.bound_at_revision,bound_at=now()",
            &[&key, &namespace, &name, &i64::from(revision), &target.sql.fingerprint, &source_revision]
        )?;
        if let Some(old) = old {
            let fingerprint: String = old.get(1);
            let referenced: bool = tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM cantongrid.model_bindings WHERE stream_key=$1 AND fingerprint=$2)",
                &[&key, &fingerprint]
            )?.get(0);
            if !referenced {
                let previous = bound
                    .iter()
                    .find(|m| m.sql.fingerprint == fingerprint)
                    .ok_or(StoreError::ModelCorrupt)?;
                clear(&mut tx, previous, &key)?;
            }
        }
        before_commit(&mut tx)?;
        if !authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(ModelBinding {
            changed: true,
            input_rows,
            projected_rows: u32::try_from(projected_rows).map_err(|_| StoreError::Limit)?,
            source_revision: source_revision.to_string(),
        })
    }
}

/// Bound cleanup too: a small backfill must not hide an unbounded DELETE of old projection rows.
fn clear(
    tx: &mut Transaction<'_>,
    model: &installed::Installed,
    stream: &str,
) -> Result<(), StoreError> {
    let count: i64 = tx.query_one(&format!(
        "SELECT count(*) FROM (SELECT 1 FROM {} WHERE \"_cg_stream_key\"=$1 LIMIT 129) bounded", model.relation()
    ), &[&stream])?.get(0);
    if count > MAX_BACKFILL_ROWS {
        return Err(StoreError::Limit);
    }
    tx.execute(
        &format!(
            "DELETE FROM {} WHERE \"_cg_stream_key\"=$1",
            model.relation()
        ),
        &[&stream],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
