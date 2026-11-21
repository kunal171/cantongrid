//! SQL-native live materialization: bounded pages, durable deduplication and no full-journal replay.
use crate::{Store, StoreError, journal::stream_key, model_projection};
use cantongrid_core::{
    authorization::SourceLease,
    config::Plan,
    lifecycle::{Action, Batch},
    value::Value,
};
use cantongrid_postgres::postgres::GenericClient;
use std::{collections::BTreeSet, time::Instant};

impl Store {
    /// Read the acknowledged cursor without rebuilding all history. Check the plan fingerprint
    /// and journal tail so a corrupted/mismatched cursor cannot silently skip source events.
    pub fn checkpoint(&mut self, plan: &Plan) -> Result<Option<String>, StoreError> {
        Ok(prefix(&mut self.client, plan)?.and_then(|(_, checkpoint)| checkpoint))
    }

    /// Record a successful no-change source observation without fabricating an event or
    /// moving the durable cursor. The source reader must have just verified the same head.
    pub fn observe_authorized(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
    ) -> Result<(), StoreError> {
        let valid = || model_projection::authorized(plan, lease);
        if !valid() {
            return Err(StoreError::Authorization);
        }
        let mut tx = self.client.transaction()?;
        model_projection::lock(&mut tx, plan)?;
        let (revision, _) = prefix(&mut tx, plan)?.ok_or(StoreError::CorruptJournal)?;
        model_projection::load_bound(&mut tx, plan, revision)?;
        let key = stream_key(plan).0;
        tx.execute(
            "UPDATE cantongrid.streams SET observed_at=now() WHERE stream_key=$1",
            &[&key],
        )?;
        if !valid() {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)
    }

    /// Publish only if the acknowledged cursor still equals the one used to read this page.
    /// This prevents a slower worker's empty page from rewinding a newer worker's progress.
    pub fn apply_page(
        &mut self,
        plan: &Plan,
        batch: &Batch,
        lease: &SourceLease,
        expected: Option<&str>,
    ) -> Result<bool, StoreError> {
        let expected = expected.map(str::to_owned);
        self.apply_live_checked(plan, batch, lease, Some(&expected))
    }

    /// Unlike the bounded fixture reference, live state uses row-level lifecycle checks. The
    /// historical journal remains durable but is never loaded wholesale into worker memory.
    pub(crate) fn apply_live(
        &mut self,
        plan: &Plan,
        batch: &Batch,
        lease: &SourceLease,
    ) -> Result<bool, StoreError> {
        self.apply_live_checked(plan, batch, lease, None)
    }

    /// Check lease/plan/checkpoint, then commit canonical events, model changes and rebuild dirty IDs together.
    fn apply_live_checked(
        &mut self,
        plan: &Plan,
        batch: &Batch,
        lease: &SourceLease,
        expected_cursor: Option<&Option<String>>,
    ) -> Result<bool, StoreError> {
        let scope = plan.context().binding();
        let expected: BTreeSet<_> = plan.source_parties().cloned().collect();
        let allowed = || {
            lease
                .source_parties(&scope, Instant::now())
                .is_some_and(|p| p == &expected)
        };
        if !allowed() {
            return Err(StoreError::Authorization);
        }
        if batch.context != *plan.context()
            || batch.events.len() > 64
            || batch.position.len() > 4096
        {
            return Err(StoreError::InvalidBatch);
        }
        let encoded = batch.encode().map_err(|_| StoreError::InvalidBatch)?;
        if encoded.len() > 1024 * 1024 {
            return Err(StoreError::Limit);
        }
        let document: serde_json::Value =
            serde_json::from_str(&encoded).map_err(|_| StoreError::InvalidBatch)?;
        let (key, lock) = stream_key(plan);
        let mut tx = self.client.transaction()?;
        if !tx
            .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&lock])?
            .get::<_, bool>(0)
        {
            return Err(StoreError::Busy);
        }
        let existing = tx.query_opt(
            "SELECT definition,revision,checkpoint FROM cantongrid.streams WHERE stream_key=$1",
            &[&key],
        )?;
        if let Some(expected) = expected_cursor {
            let actual = existing
                .as_ref()
                .and_then(|r| r.get::<_, Option<String>>(2));
            if &actual != expected {
                return Err(StoreError::Busy);
            }
        }
        let revision = if let Some(row) = existing {
            if row.get::<_, serde_json::Value>(0) != plan.definition() {
                return Err(StoreError::PlanChanged);
            }
            row.get::<_, i64>(1)
        } else {
            0
        };
        if let Some(row) = tx.query_opt(
            "SELECT envelope FROM cantongrid.batches WHERE stream_key=$1 AND position=$2",
            &[&key, &batch.position],
        )? {
            return if row.get::<_, serde_json::Value>(0) == document {
                Ok(false)
            } else {
                Err(StoreError::InvalidBatch)
            };
        }
        tx.execute("INSERT INTO cantongrid.streams(stream_key,definition) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&key,&plan.definition()])?;
        let models = model_projection::load_bound(&mut tx, plan, revision)?;
        persist_events(&mut tx, plan, batch, &models)?;
        let next = revision.checked_add(1).ok_or(StoreError::Limit)?;
        tx.execute("INSERT INTO cantongrid.batches(stream_key,revision,position,envelope) VALUES($1,$2,$3,$4)",&[&key,&next,&batch.position,&document])?;
        tx.execute("UPDATE cantongrid.streams SET revision=$2,checkpoint=$3,observed_at=now() WHERE stream_key=$1",&[&key,&next,&batch.position])?;
        if !allowed() {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(true)
    }
}

/// Verify a source prefix within either the operator's writer transaction or a checkpoint read.
/// No artificial revision-zero stream can be used to activate projections before ingestion.
pub(crate) fn prefix(
    client: &mut impl GenericClient,
    plan: &Plan,
) -> Result<Option<(i64, Option<String>)>, StoreError> {
    let key = stream_key(plan).0;
    let row = client.query_opt("SELECT definition,revision,checkpoint,(SELECT position FROM cantongrid.batches WHERE stream_key=$1 ORDER BY revision DESC LIMIT 1),(SELECT max(revision) FROM cantongrid.batches WHERE stream_key=$1) FROM cantongrid.streams WHERE stream_key=$1", &[&key])?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.get::<_, serde_json::Value>(0) != plan.definition() {
        return Err(StoreError::PlanChanged);
    }
    let checkpoint: Option<String> = row.get(2);
    let revision: i64 = row.get(1);
    if checkpoint != row.get::<_, Option<String>>(3)
        || row.get::<_, Option<i64>>(4) != Some(revision)
    {
        return Err(StoreError::CorruptJournal);
    }
    Ok(Some((revision, checkpoint)))
}

/// Persist a bounded batch inside the caller's writer transaction. Upgrade baselines use
/// exactly the same lifecycle, projection, history and outbox checks as ordinary ingestion.
pub(crate) fn persist_events(
    tx: &mut cantongrid_postgres::postgres::Transaction<'_>,
    plan: &Plan,
    batch: &Batch,
    models: &[model_projection::installed::Installed],
) -> Result<(), StoreError> {
    let key = stream_key(plan).0;
    let expected: BTreeSet<_> = plan.source_parties().cloned().collect();
    let mut budget = model_projection::Budget::default();
    let mut seen = BTreeSet::new();
    for event in &batch.events {
        if event.id.is_empty()
            || event.contract.is_empty()
            || !seen.insert(&event.id)
            || !plan.selects(&event.template)
            || event.template.validate().is_err()
            || event.observed_at_micros < 0
            || event.ledger_at_micros.is_some_and(|t| t < 0)
            || event.origin.as_ref().is_some_and(|o| o.offset < 0)
            || event.id.chars().any(char::is_control)
            || event.contract.chars().any(char::is_control)
        {
            return Err(StoreError::InvalidBatch);
        }
        let witnesses: Vec<_> = event
            .witnesses
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        let (kind, clauses) = match &event.action {
            Action::Create(Value::RawJson(payload)) => {
                Value::RawJson(payload.clone())
                    .validate()
                    .map_err(|_| StoreError::InvalidBatch)?;
                if event.witnesses.is_empty() || event.witnesses.is_disjoint(&expected) {
                    return Err(StoreError::Authorization);
                }
                if tx
                    .query_opt(
                        "SELECT 1 FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2",
                        &[&key, &event.contract],
                    )?
                    .is_some()
                {
                    return Err(StoreError::InvalidBatch);
                }
                let template =
                    serde_json::to_value(&event.template).map_err(|_| StoreError::InvalidBatch)?;
                let value = serde_json::to_value(Value::RawJson(payload.clone()))
                    .map_err(|_| StoreError::InvalidBatch)?;
                tx.execute("INSERT INTO cantongrid.contracts(stream_key,contract_id,template,payload,create_event_id,witnesses,active) VALUES($1,$2,$3,$4,$5,$6,true)",&[&key,&event.contract,&template,&value,&event.id,&witnesses])?;
                model_projection::apply(
                    tx,
                    models,
                    model_projection::Input {
                        template: &event.template,
                        stream: &key,
                        contract: &event.contract,
                        payload,
                        archive: false,
                    },
                    &mut budget,
                )?;
                ("create", serde_json::json!([witnesses]))
            }
            Action::ArchiveByPriorContract => {
                if !witnesses.is_empty() {
                    return Err(StoreError::InvalidBatch);
                }
                let prior=tx.query_opt("SELECT template,witnesses,payload FROM cantongrid.contracts WHERE stream_key=$1 AND contract_id=$2 AND active",&[&key,&event.contract])?.ok_or(StoreError::InvalidBatch)?;
                if prior.get::<_, serde_json::Value>(0)
                    != serde_json::to_value(&event.template)
                        .map_err(|_| StoreError::InvalidBatch)?
                {
                    return Err(StoreError::InvalidBatch);
                }
                let original: Vec<String> = prior.get(1);
                if !original
                    .iter()
                    .any(|p| expected.iter().any(|e| e.as_str() == p))
                {
                    return Err(StoreError::Authorization);
                }
                tx.execute("UPDATE cantongrid.contracts SET active=false,archive_event_id=$3 WHERE stream_key=$1 AND contract_id=$2",&[&key,&event.contract,&event.id])?;
                let Value::RawJson(payload) =
                    serde_json::from_value(prior.get(2)).map_err(|_| StoreError::InvalidBatch)?
                else {
                    return Err(StoreError::InvalidBatch);
                };
                model_projection::apply(
                    tx,
                    models,
                    model_projection::Input {
                        template: &event.template,
                        stream: &key,
                        contract: &event.contract,
                        payload: &payload,
                        archive: true,
                    },
                    &mut budget,
                )?;
                ("archive", serde_json::json!([original]))
            }
            _ => return Err(StoreError::InvalidBatch),
        };
        model_projection::rebuild::changed(tx, plan, &event.contract, &event.template)?;
        let single = Batch {
            context: batch.context.clone(),
            position: batch.position.clone(),
            events: vec![event.clone()],
        };
        let envelope: serde_json::Value =
            serde_json::from_str(&single.encode().map_err(|_| StoreError::InvalidBatch)?)
                .map_err(|_| StoreError::InvalidBatch)?;
        tx.execute("INSERT INTO cantongrid.events(stream_key,event_id,contract_id,position,kind,envelope,visibility_clauses) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&key,&event.id,&event.contract,&batch.position,&kind,&envelope,&clauses])?;
        tx.execute(
            "INSERT INTO cantongrid.outbox(stream_key,event_id) VALUES($1,$2)",
            &[&key, &event.id],
        )?;
    }
    Ok(())
}
