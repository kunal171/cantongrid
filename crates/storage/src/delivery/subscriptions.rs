//! Registration and explicit ACKs never modify canonical events, projections or source progress.
use super::*;
impl crate::Store {
    /// Retry registration by name without resetting progress. New policy bindings cannot inherit it.
    pub fn subscribe(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        request: SubscriptionRequest<'_>,
    ) -> Result<Subscription, StoreError> {
        let current = parties(plan, caller, lease)?;
        if request.name.is_empty()
            || request.name.len() > 64
            || !request.name.as_bytes()[0].is_ascii_alphabetic()
            || !request
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(StoreError::ModelQuery);
        }
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        tx.execute("DELETE FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND (expires_at<=now() OR (principal_fingerprint=$2 AND authorization_binding<>$3))",&[&key,&consumer.principal,&consumer.binding])?;
        let existing=tx.query_opt("SELECT id,start_mode FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND principal_fingerprint=$2 AND authorization_binding=$3 AND client_name=$4",&[&key,&consumer.principal,&consumer.binding,&request.name])?;
        let id: String = if let Some(row) = existing {
            if row.get::<_, String>(1) != request.start.as_str() {
                return Err(StoreError::ModelQuery);
            }
            row.get(0)
        } else {
            let row=tx.query_one("SELECT count(*),count(*) FILTER(WHERE principal_fingerprint=$2) FROM cantongrid.delivery_subscriptions WHERE stream_key=$1",&[&key,&consumer.principal])?;
            if row.get::<_, i64>(0) >= 256 || row.get::<_, i64>(1) >= 16 {
                return Err(StoreError::DeliveryBackpressure);
            }
            let row=tx.query_one("SELECT min(delivery_id),max(delivery_id) FROM cantongrid.outbox WHERE stream_key=$1",&[&key])?;
            let anchor = match request.start {
                DeliveryStart::Now => row.get::<_, Option<i64>>(1).unwrap_or(0),
                DeliveryStart::Retained => row.get::<_, Option<i64>>(0).map_or(0, |n| n - 1),
            };
            tx.query_one("INSERT INTO cantongrid.delivery_subscriptions(stream_key,principal_fingerprint,authorization_binding,client_name,start_mode,parties,acknowledged) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING id",&[&key,&consumer.principal,&consumer.binding,&request.name,&request.start.as_str(),&current,&anchor])?.get(0)
        };
        let acknowledged = state(&mut tx, &key, consumer, &id, &current)?;
        let resume = receipt(&mut tx, &id, acknowledged, acknowledged)?;
        let expires:i64=tx.query_one("SELECT extract(epoch FROM expires_at)::bigint FROM cantongrid.delivery_subscriptions WHERE id=$1",&[&id])?.get(0);
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(Subscription {
            id,
            receipt: resume,
            start: request.start,
            expires_at_seconds: expires.to_string(),
        })
    }
    /// ACK a processed contiguous prefix. Late ACKs are monotonic; a retry never rewinds progress.
    pub fn acknowledge(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        id: &str,
        resume: &str,
    ) -> Result<(), StoreError> {
        let current = parties(plan, caller, lease)?;
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        let old = state(&mut tx, &key, consumer, id, &current)?;
        let after = position(&mut tx, id, resume)?;
        tx.execute(
            "UPDATE cantongrid.delivery_subscriptions SET acknowledged=$2 WHERE id=$1",
            &[&id, &after.max(old)],
        )?;
        // Retain a bounded retry window, including ordinary out-of-order acknowledgements.
        tx.execute("DELETE FROM cantongrid.delivery_receipts WHERE id IN(SELECT id FROM cantongrid.delivery_receipts WHERE subscription_id=$1 AND after_delivery<=$2 ORDER BY after_delivery DESC,id OFFSET 64)",&[&id,&after.max(old)])?;
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)
    }
    /// Cancellation is caller-owned; unknown IDs and other principals have the same failure.
    pub fn cancel_subscription(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        id: &str,
    ) -> Result<(), StoreError> {
        let current = parties(plan, caller, lease)?;
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        state(&mut tx, &key, consumer, id, &current)?;
        tx.execute(
            "DELETE FROM cantongrid.delivery_subscriptions WHERE id=$1",
            &[&id],
        )?;
        parties(plan, caller, lease)?;
        tx.commit().map_err(|_| StoreError::CommitUnknown)
    }
}
