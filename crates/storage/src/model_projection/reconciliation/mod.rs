//! Read-only exact per-contract checks over one complete retained indexed prefix and active model.
mod report;
use super::{BoundModel, installed, relationships::active};
use crate::{Store, StoreError, journal::stream_key, source_position::PqsPosition};
use cantongrid_core::{
    authorization::SourceLease, config::Plan, value::Value, visibility::CallerGrants,
};
use cantongrid_models::reconciliation::Policy;
use cantongrid_postgres::postgres::{IsolationLevel, Transaction};
use report::Counter;
pub use report::ReconciliationReport;
use std::time::Instant;

const MAX_INPUTS: i64 = 128;
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Check complete caller scope independently from input membership, before any data or count query.
fn parties(
    plan: &Plan,
    caller: &CallerGrants,
    lease: &SourceLease,
) -> Result<Vec<String>, StoreError> {
    if !super::authorized(plan, lease) {
        return Err(StoreError::Authorization);
    }
    let parties = lease
        .read_parties(&plan.context().binding(), caller, Instant::now())
        .ok_or(StoreError::Authorization)?;
    if parties.len() != plan.source_parties().count() {
        return Err(StoreError::Authorization);
    }
    Ok(parties
        .iter()
        .map(|party| party.as_str().to_owned())
        .collect())
}
impl Store {
    /// Evaluate one exact active model with current trusted caller/source grants; never writes or repairs.
    /// This proves only the bounded retained indexed prefix, not asset units or the current ledger head.
    pub fn read_reconciliation(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        policy: &Policy,
    ) -> Result<ReconciliationReport, StoreError> {
        self.read_reconciliation_checked(plan, caller, lease, binding, policy, || Ok(()))
    }

    /// A private barrier exercises concurrent ingestion/promotion and lease expiry after snapshot acquisition.
    fn read_reconciliation_checked(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        binding: &BoundModel,
        policy: &Policy,
        after_snapshot: impl FnOnce() -> Result<(), StoreError>,
    ) -> Result<ReconciliationReport, StoreError> {
        let allowed = parties(plan, caller, lease)?;
        let result = (|| {
            if !plan.is_live() || plan.source_binding().is_none() {
                return Err(StoreError::ReconciliationUnavailable);
            }
            let mut tx = self
                .client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()?;
            let (revision, checkpoint) =
                crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::ReconciliationUnavailable)?;
            after_snapshot()?;
            let checkpoint = checkpoint.ok_or(StoreError::ReconciliationUnavailable)?;
            let boundary = PqsPosition::decode(&checkpoint)?
                .complete_boundary()
                .ok_or(StoreError::ReconciliationUnavailable)?;
            let key = stream_key(plan).0;
            if !crate::operations::fresh(&mut tx, &key)? {
                return Err(StoreError::ReconciliationUnavailable);
            }
            crate::temporal::coverage::observed(&mut tx, &key, revision, &checkpoint, boundary)?;
            let model = active(&mut tx, plan, binding)?;
            let preflight = policy
                .preflight_model(&binding.namespace, &model.model)
                .map_err(|_| StoreError::ModelQuery)?;
            let (contracts, counters) = evaluate(&mut tx, &key, &allowed, &model, policy)?;
            // Recheck on both sides of COMMIT so a completed read cannot outlive its source evidence.
            if parties(plan, caller, lease)? != allowed {
                return Err(StoreError::Authorization);
            }
            tx.commit()?;
            if parties(plan, caller, lease)? != allowed {
                return Err(StoreError::Authorization);
            }
            Ok(ReconciliationReport::complete(
                binding,
                policy,
                preflight.fingerprint().into(),
                contracts,
                counters,
            ))
        })();
        // Do not reveal which coverage, registry or projection check failed, or return partial counts.
        result.map_err(|error| {
            if error == StoreError::Authorization {
                error
            } else {
                StoreError::ReconciliationUnavailable
            }
        })
    }
}

/// Bound the canonical set before model filtering; dropping a missing projection must not hide a liability.
fn evaluate(
    tx: &mut Transaction<'_>,
    key: &str,
    parties: &[String],
    model: &installed::Installed,
    policy: &Policy,
) -> Result<(u32, Vec<Counter>), StoreError> {
    let template =
        serde_json::to_value(&model.model.template).map_err(|_| StoreError::ModelCorrupt)?;
    let bound = MAX_INPUTS + 1;
    // Measure before transferring payloads. Even a corrupt enormous input cannot allocate an unbounded row set.
    let size = tx.query_one("SELECT count(*),COALESCE(sum(octet_length(payload::text)+octet_length(contract_id)),0)::bigint,coalesce(bool_and(witnesses && $3::text[]),false) FROM (SELECT contract_id,payload,witnesses FROM cantongrid.contracts WHERE stream_key=$1 AND active AND template=$2 LIMIT $4) bounded", &[&key,&template,&parties,&bound])?;
    let count: i64 = size.get(0);
    let mut bytes = usize::try_from(size.get::<_, i64>(1)).map_err(|_| StoreError::Limit)?;
    if count > MAX_INPUTS || bytes > MAX_BYTES || (count > 0 && !size.get::<_, bool>(2)) {
        return Err(StoreError::ReconciliationUnavailable);
    }
    let relation = model.relation();
    let projected: i64 = tx.query_one(&format!("SELECT count(*) FROM (SELECT 1 FROM {relation} WHERE \"_cg_stream_key\"=$1 LIMIT $2) bounded"), &[&key,&bound])?.get(0);
    if projected > MAX_INPUTS {
        return Err(StoreError::ReconciliationUnavailable);
    }
    let rows = tx.query("SELECT contract_id,payload FROM cantongrid.contracts WHERE stream_key=$1 AND active AND template=$2 ORDER BY contract_id COLLATE \"C\" LIMIT $3", &[&key,&template,&bound])?;
    let mut counters: Vec<Counter> = policy.check_names().map(|_| Counter::default()).collect();
    let mut selected = 0;
    for row in rows {
        let contract: String = row.get(0);
        let Value::RawJson(payload) =
            serde_json::from_value(row.get(1)).map_err(|_| StoreError::ModelProjection)?
        else {
            return Err(StoreError::ModelProjection);
        };
        let fields = model
            .model
            .evaluate(&payload)
            .map_err(|_| StoreError::ModelProjection)?;
        if let Some(fields) = fields {
            let expected = model
                .model
                .row_json(key, &contract, &fields)
                .map_err(|_| StoreError::ModelProjection)?;
            let encoded =
                serde_json::to_string(&expected).map_err(|_| StoreError::ModelProjection)?;
            bytes = bytes.checked_add(encoded.len()).ok_or(StoreError::Limit)?;
            if bytes > MAX_BYTES {
                return Err(StoreError::Limit);
            }
            // Native row equality compares exact numeric/bigint columns, never JSON float approximations.
            let same: bool = tx.query_one(&format!("SELECT EXISTS(SELECT 1 FROM {relation} a,jsonb_populate_record(NULL::{relation},$3) e WHERE a.\"_cg_stream_key\"=$1 AND a.\"_cg_contract_id\"=$2 AND a IS NOT DISTINCT FROM e)"), &[&key,&contract,&expected])?.get(0);
            if !same {
                return Err(StoreError::ReconciliationUnavailable);
            }
            let findings = policy.evaluate_fixture(&encoded);
            for (counter, finding) in counters.iter_mut().zip(findings.checks()) {
                counter.add(finding.fixture_comparison);
            }
            selected += 1;
        }
    }
    // Equal counts plus every expected native row excludes extra archived, filtered or unrelated contributions.
    if selected == 0 || i64::from(selected) != projected {
        return Err(StoreError::ReconciliationUnavailable);
    }
    Ok((selected, counters))
}
