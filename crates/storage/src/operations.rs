//! Full-source operator counters are never returned through party-scoped data APIs.
use crate::{Store, StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan};
use cantongrid_postgres::postgres::{GenericClient, IsolationLevel};
/// Only ingestion updates this heartbeat; consumer reads/ACKs must not manufacture freshness.
pub(crate) fn fresh(client: &mut impl GenericClient, key: &str) -> Result<bool, StoreError> {
    Ok(client.query_opt("SELECT observed_at>=now()-interval '30 seconds' FROM cantongrid.streams WHERE stream_key=$1",&[&key])?.is_some_and(|r|r.get(0)))
}
impl Store {
    /// A healthy source alone is insufficient when the ingestion worker stopped observing progress.
    pub fn ready_for_live_delivery(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
    ) -> Result<bool, StoreError> {
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let ready =
            crate::live::prefix(&mut tx, plan)?.is_some() && fresh(&mut tx, &stream_key(plan).0)?;
        tx.commit()?;
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        Ok(ready)
    }

    /// Observe owned queue pressure in a read-only snapshot under exact current source authority.
    pub fn operational_metrics(
        &mut self,
        plan: &Plan,
        lease: &SourceLease,
    ) -> Result<serde_json::Value, StoreError> {
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        crate::live::prefix(&mut tx, plan)?.ok_or(StoreError::InspectorUnavailable)?;
        let key = stream_key(plan).0;
        let row=tx.query_one("SELECT (SELECT count(*) FROM cantongrid.outbox WHERE stream_key=$1),(SELECT count(*) FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND expires_at>now()),(SELECT count(*) FROM cantongrid.delivery_receipts r JOIN cantongrid.delivery_subscriptions s ON s.id=r.subscription_id WHERE s.stream_key=$1 AND s.expires_at>now() AND r.created_at>=now()-interval '5 minutes' AND r.after_delivery>s.acknowledged),(SELECT count(*) FROM cantongrid.delivery_subscriptions WHERE stream_key=$1 AND expires_at>now() AND lost_through>acknowledged)",&[&key])?;
        let observed=tx.query_one("SELECT GREATEST(0,extract(epoch FROM now()-observed_at)::bigint),observed_at>=now()-interval '30 seconds' FROM cantongrid.streams WHERE stream_key=$1",&[&key])?;
        let report = serde_json::json!({"ingestionObservationAgeSeconds":observed.get::<_,i64>(0).to_string(),"ingestionFresh":observed.get::<_,bool>(1),"outboxRows":row.get::<_,i64>(0).to_string(),"activeSubscriptions":row.get::<_,i64>(1).to_string(),"unacknowledgedReceipts":row.get::<_,i64>(2).to_string(),"subscriptionsWithRecordedVisibleLoss":row.get::<_,i64>(3).to_string(),"workerRetentionTargetEvents":"100000","retentionDeleteLimitPerCycle":256,"subscriptionTtlSeconds":"86400","receiptTtlSeconds":"300","signedCursorMaxSeconds":"60","counterScope":"owned_destination_full_configured_source"});
        tx.commit()?;
        if !crate::model_projection::authorized(plan, lease) {
            return Err(StoreError::Authorization);
        }
        Ok(report)
    }
}
