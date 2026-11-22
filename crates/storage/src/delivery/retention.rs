//! Retention classifies loss using each consumer's own authorization snapshot, never global gaps.
use super::*;
use crate::Store;
use cantongrid_core::visibility::{PartyId, VisibilityPolicy};

/// Reuse the core AND-of-OR proof evaluator; malformed evidence is not permission to delete it.
fn policy(plan: &Plan, value: serde_json::Value) -> Result<VisibilityPolicy, StoreError> {
    let clauses: Vec<Vec<String>> =
        serde_json::from_value(value).map_err(|_| StoreError::CorruptJournal)?;
    if clauses.is_empty()
        || clauses.len() > 16
        || clauses.iter().any(|c| c.is_empty() || c.len() > 128)
    {
        return Err(StoreError::CorruptJournal);
    }
    let inputs = clauses
        .into_iter()
        .map(|clause| {
            let parties = clause
                .into_iter()
                .map(PartyId::new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| StoreError::CorruptJournal)?;
            VisibilityPolicy::from_witnesses(plan.context().binding(), parties)
                .map_err(|_| StoreError::CorruptJournal)
        })
        .collect::<Result<Vec<_>, _>>()?;
    VisibilityPolicy::join(&inputs).map_err(|_| StoreError::CorruptJournal)
}
impl Store {
    /// Remove at most 256 old outbox records, preserving canonical history and source checkpoints.
    /// The worker retains 100,000 events by default; operators may choose a smaller explicit budget.
    pub fn prune_deliveries(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
        keep: usize,
        limit: usize,
    ) -> Result<usize, StoreError> {
        if !plan.is_live()
            || plan.source_binding().is_none()
            || !crate::model_projection::authorized(plan, lease)
        {
            return Err(StoreError::Authorization);
        }
        if !(1..=100000).contains(&keep) || !(1..=256).contains(&limit) {
            return Err(StoreError::ModelQuery);
        }
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        tx.execute("DELETE FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND expires_at<=now()",&[&key])?;
        let cutoff=tx.query_opt("SELECT delivery_id FROM cantongrid.outbox WHERE stream_key=$1 ORDER BY delivery_id DESC OFFSET $2 LIMIT 1",&[&key,&(keep as i64)])?.map(|r|r.get::<_,i64>(0));
        let rows = if let Some(cutoff) = cutoff {
            tx.query("SELECT o.delivery_id,CASE WHEN octet_length(e.visibility_clauses::text)<=65536 THEN e.visibility_clauses END FROM cantongrid.outbox o JOIN cantongrid.events e USING(stream_key,event_id) WHERE o.stream_key=$1 AND o.delivery_id<=$2 ORDER BY o.delivery_id LIMIT $3",&[&key,&cutoff,&(limit as i64)])?
        } else {
            vec![]
        };
        let mut budget = 4 * 1024 * 1024usize;
        let mut removed = Vec::new();
        let mut proofs = Vec::new();
        for row in rows {
            let value: serde_json::Value = row
                .get::<_, Option<serde_json::Value>>(1)
                .ok_or(StoreError::Limit)?;
            budget = budget
                .checked_sub(value.to_string().len())
                .ok_or(StoreError::Limit)?;
            let after: i64 = row.get(0);
            removed.push(after);
            proofs.push((after, policy(plan, value)?));
        }
        let subscriptions=tx.query("SELECT id,acknowledged,parties FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 LIMIT 257",&[&key])?;
        if subscriptions.len() > 256 {
            return Err(StoreError::Limit);
        }
        for row in subscriptions {
            let id: String = row.get(0);
            let acknowledged: i64 = row.get(1);
            let stored: Vec<String> = row.get(2);
            let caller = CallerGrants::from_verified(
                plan.context().binding(),
                stored
                    .into_iter()
                    .map(PartyId::new)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| StoreError::CorruptJournal)?,
            );
            let lost = proofs
                .iter()
                .filter(|(after, proof)| *after > acknowledged && proof.allows(&caller))
                .map(|(after, _)| *after)
                .max();
            if let Some(lost) = lost {
                tx.execute("UPDATE cantongrid.delivery_subscriptions SET lost_through=GREATEST(lost_through,$2) WHERE id=$1",&[&id,&lost])?;
            }
        }
        tx.execute(
            "DELETE FROM cantongrid.outbox WHERE stream_key=$1 AND delivery_id=ANY($2::bigint[])",
            &[&key, &removed],
        )?;
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(removed.len())
    }
}
