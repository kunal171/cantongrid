//! Bounded at-least-once reads; hidden activity changes neither payloads nor public cursor positions.
use super::*;
use crate::Store;

/// Caller-supplied API inputs after signature verification; receipt IDs are opaque random values.
pub struct DeliveryQuery<'a> {
    pub subscription: &'a str,
    pub after: Option<&'a str>,
    pub limit: usize,
}
/// Only event-local records leave storage; the internal delivery sequence is discarded.
pub struct DeliveryPage {
    pub events: Vec<serde_json::Value>,
    pub receipt: Option<String>,
    pub more: bool,
}
impl Store {
    /// Without a cursor, resume the durable ACK. With one, resume its issued prefix without rewinding ACKs.
    pub fn read_deliveries(
        &mut self,
        plan: &Plan,
        caller: &CallerGrants,
        lease: &SourceLease,
        consumer: &Consumer,
        query: DeliveryQuery<'_>,
    ) -> Result<DeliveryPage, StoreError> {
        let current = parties(plan, caller, lease)?;
        if !(1..=25).contains(&query.limit) {
            return Err(StoreError::ModelQuery);
        }
        let mut tx = self.client.transaction()?;
        let key = lock(&mut tx, plan)?;
        if !crate::operations::fresh(&mut tx, &key)? {
            return Err(StoreError::DeliveryStale);
        }
        let acknowledged = state(&mut tx, &key, consumer, query.subscription, &current)?;
        let after = match query.after {
            Some(resume) => position(&mut tx, query.subscription, resume)?.max(acknowledged),
            None => acknowledged,
        };
        let lost: i64 = tx
            .query_one(
                "SELECT lost_through FROM cantongrid.delivery_subscriptions WHERE id=$1",
                &[&query.subscription],
            )?
            .get(0);
        if after < lost {
            return Err(StoreError::DeliveryGap);
        }
        let mut rows = tx.query(
            include_str!("read.sql"),
            &[&key, &after, &current, &(query.limit as i64 + 1)],
        )?;
        let more = rows.len() > query.limit;
        rows.truncate(query.limit);
        let resume = rows
            .last()
            .map(|row| receipt(&mut tx, query.subscription, row.get(0), acknowledged))
            .transpose()?;
        let events=rows.into_iter().map(|row|serde_json::json!({"eventId":row.get::<_,String>(1),"contractId":row.get::<_,String>(2),"kind":row.get::<_,String>(3),"envelope":row.get::<_,serde_json::Value>(4)})).collect();
        if parties(plan, caller, lease)? != current {
            return Err(StoreError::Authorization);
        }
        tx.commit().map_err(|_| StoreError::CommitUnknown)?;
        Ok(DeliveryPage {
            events,
            receipt: resume,
            more,
        })
    }
}
