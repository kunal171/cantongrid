//! A fixed private snapshot boundary and its feed anchor are committed under the ingestion lock.
use super::*;
use crate::{ReadPage, Store, TemporalQuery};

/// Load only an owned baseline that has not advanced beyond its original delivery anchor.
/// Baseline reads never ACK; a client cannot replay an old baseline over newer committed effects.
fn pinned(
    tx: &mut Transaction<'_>,
    key: &str,
    consumer: &Consumer,
    id: &str,
    current: &[String],
) -> Result<i64, StoreError> {
    let acknowledged = state(tx, key, consumer, id, current)?;
    let row = tx.query_one("SELECT baseline_offset,baseline_after,lost_through FROM cantongrid.delivery_subscriptions WHERE id=$1", &[&id])?;
    let boundary = row
        .get::<_, Option<i64>>(0)
        .ok_or(StoreError::BootstrapConflict)?;
    let anchor = row
        .get::<_, Option<i64>>(1)
        .ok_or(StoreError::BootstrapConflict)?;
    if acknowledged != anchor {
        return Err(StoreError::BootstrapConflict);
    }
    if row.get::<_, i64>(2) > anchor {
        return Err(StoreError::DeliveryGap);
    }
    Ok(boundary)
}

/// Check the same bounded journal and immutable membership evidence used by canonical temporal reads.
fn coverage(
    tx: &mut Transaction<'_>,
    plan: &Plan,
    key: &str,
    offset: i64,
) -> Result<(), StoreError> {
    let (revision, checkpoint) =
        crate::live::prefix(tx, plan)?.ok_or(StoreError::TemporalUnavailable)?;
    let checkpoint = checkpoint.ok_or(StoreError::TemporalUnavailable)?;
    crate::temporal::coverage::observed(tx, key, revision, &checkpoint, offset)?;
    Ok(())
}

impl Store {
    /// Atomically pair a proven complete canonical baseline with a durable private feed position.
    /// The returned subscription is opaque: the source offset stays in owned storage, never in JWTs.
    pub fn bootstrap_subscription(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        client_name: &str,
    ) -> Result<Subscription, StoreError> {
        let current = parties(plan, caller, lease)?;
        name(client_name)?;
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan).map_err(|error| {
            if error == StoreError::SubscriptionUnavailable {
                StoreError::TemporalUnavailable
            } else {
                error
            }
        })?;
        if !crate::operations::fresh(&mut tx, &key)? {
            return Err(StoreError::DeliveryStale);
        }
        // Look up before expiry cleanup: a retained expired pairing must not silently become a new baseline.
        let existing = tx.query_opt("SELECT id,expires_at>now() FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND principal_fingerprint=$2 AND authorization_binding=$3 AND client_name=$4", &[&key,&consumer.principal,&consumer.binding,&client_name])?;
        let id: String = if let Some(row) = existing {
            if !row.get::<_, bool>(1) {
                return Err(StoreError::SubscriptionUnavailable);
            }
            row.get(0)
        } else {
            cleanup(&mut tx, &key, consumer)?;
            capacity(&mut tx, &key, consumer)?;
            let (_, checkpoint) =
                crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::TemporalUnavailable)?;
            let checkpoint = checkpoint.ok_or(StoreError::TemporalUnavailable)?;
            let boundary = crate::source_position::PqsPosition::decode(&checkpoint)?
                .complete_boundary()
                .ok_or(StoreError::TemporalUnavailable)?;
            let anchor: i64 = tx.query_one("SELECT COALESCE(max(delivery_id),0) FROM cantongrid.outbox WHERE stream_key=$1", &[&key])?.get(0);
            // All effects through B are complete. Writers/pruning cannot interleave before this row commits.
            tx.query_one("INSERT INTO cantongrid.delivery_subscriptions(stream_key,principal_fingerprint,authorization_binding,client_name,start_mode,parties,acknowledged,baseline_offset,baseline_after) VALUES($1,$2,$3,$4,'now',$5,$6,$7,$6) RETURNING id", &[&key,&consumer.principal,&consumer.binding,&client_name,&current,&anchor,&boundary])?.get(0)
        };
        let boundary = pinned(&mut tx, &key, consumer, &id, &current)?;
        coverage(&mut tx, plan, &key, boundary)?;
        let acknowledged = state(&mut tx, &key, consumer, &id, &current)?;
        let resume = receipt(&mut tx, &id, acknowledged, acknowledged)?;
        let expires: i64 = tx.query_one("SELECT extract(epoch FROM expires_at)::bigint FROM cantongrid.delivery_subscriptions WHERE id=$1", &[&id])?.get(0);
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        Ok(Subscription {
            id,
            receipt: resume,
            start: DeliveryStart::Now,
            expires_at_seconds: expires.to_string(),
        })
    }

    /// Page the immutable baseline under the original consumer binding, even while newer events arrive.
    /// Private progress fields in ReadPage are for storage callers only; HTTP must redact them.
    pub fn read_bootstrap(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        query: DeliveryQuery<'_>,
    ) -> Result<ReadPage, StoreError> {
        let current = parties(plan, caller, lease)?;
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        let boundary = pinned(&mut tx, &key, consumer, query.subscription, &current)?;
        let (revision, checkpoint) =
            crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::TemporalUnavailable)?;
        let checkpoint = checkpoint.ok_or(StoreError::TemporalUnavailable)?;
        let page = crate::temporal::read_at(
            &mut tx,
            &key,
            revision,
            &checkpoint,
            &current,
            TemporalQuery {
                offset: boundary,
                after: query.after,
                limit: query.limit,
            },
        )?;
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        tx.commit()?;
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        Ok(page)
    }
}
