//! Private durable consumer progress. Public cursors use random receipts, never outbox sequence IDs.
mod read;
mod subscriptions;
use crate::{StoreError, journal::stream_key};
use cantongrid_core::{authorization::SourceLease, config::Plan, visibility::CallerGrants};
use cantongrid_postgres::postgres::Transaction;
pub use read::{DeliveryPage, DeliveryQuery};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Trusted caller fingerprints supplied by the authenticated API, never by a request body.
#[derive(Clone)]
pub struct Consumer {
    principal: String,
    binding: String,
}
impl Consumer {
    /// Accept only opaque digests; this constructor is for an already verified organization caller.
    pub fn from_verified(principal: String, binding: String) -> Result<Self, StoreError> {
        let digest = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if !digest(&principal) || !digest(&binding) {
            return Err(StoreError::Authorization);
        }
        Ok(Self { principal, binding })
    }
}
/// Starting at the retained outbox does not claim pre-snapshot or complete source history.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStart {
    Now,
    Retained,
}
impl DeliveryStart {
    fn as_str(self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::Retained => "retained",
        }
    }
}
/// A stable client name makes registration retries idempotent within the current authority binding.
pub struct SubscriptionRequest<'a> {
    pub name: &'a str,
    pub start: DeliveryStart,
}
/// These fields describe only this consumer. The random receipt must be signed by the API.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub receipt: String,
    pub start: DeliveryStart,
    pub expires_at_seconds: String,
}

/// Exact current party scope is checked again inside storage and after each transaction.
fn parties(
    plan: &Plan,
    caller: &CallerGrants,
    lease: &SourceLease,
) -> Result<Vec<String>, StoreError> {
    if !plan.is_live()
        || plan.source_binding().is_none()
        || !crate::model_projection::authorized(plan, lease)
    {
        return Err(StoreError::Authorization);
    }
    let result: Vec<_> = lease
        .read_parties(&plan.context().binding(), caller, Instant::now())
        .ok_or(StoreError::Authorization)?
        .iter()
        .map(|p| p.as_str().to_owned())
        .collect();
    if result.is_empty() {
        return Err(StoreError::Authorization);
    }
    Ok(result)
}
/// Source writer serialization also makes receipt reuse and ACK/pruning decisions atomic.
fn lock(tx: &mut Transaction<'_>, plan: &Plan) -> Result<String, StoreError> {
    crate::model_projection::lock(tx, plan)?;
    crate::live::prefix(tx, plan)?.ok_or(StoreError::SubscriptionUnavailable)?;
    Ok(stream_key(plan).0)
}
/// Avoid SQL UUID parsing errors and keep unknown/wrong-owner IDs indistinguishable.
fn identity(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
            }
        })
}
/// Load only a live, caller-owned subscription with the exact party set that was registered.
fn state(
    tx: &mut Transaction<'_>,
    key: &str,
    consumer: &Consumer,
    id: &str,
    current: &[String],
) -> Result<i64, StoreError> {
    if !identity(id) {
        return Err(StoreError::SubscriptionUnavailable);
    }
    let row=tx.query_opt("SELECT acknowledged,parties FROM cantongrid.delivery_subscriptions WHERE id=$1 AND stream_key=$2 AND principal_fingerprint=$3 AND authorization_binding=$4 AND expires_at>now()",&[&id,&key,&consumer.principal,&consumer.binding])?.ok_or(StoreError::SubscriptionUnavailable)?;
    if row.get::<_, Vec<String>>(1) != current {
        return Err(StoreError::SubscriptionUnavailable);
    }
    Ok(row.get(0))
}
/// Keep at most 64 unacknowledged positions plus 64 recent acknowledged retries per consumer.
fn receipt(
    tx: &mut Transaction<'_>,
    id: &str,
    after: i64,
    acknowledged: i64,
) -> Result<String, StoreError> {
    tx.execute("DELETE FROM cantongrid.delivery_receipts WHERE subscription_id=$1 AND created_at<now()-interval '5 minutes'",&[&id])?;
    if let Some(row)=tx.query_opt("SELECT id FROM cantongrid.delivery_receipts WHERE subscription_id=$1 AND after_delivery=$2 LIMIT 1",&[&id,&after])?{return Ok(row.get(0));}
    let pending:i64=tx.query_one("SELECT count(*) FROM cantongrid.delivery_receipts WHERE subscription_id=$1 AND after_delivery>$2",&[&id,&acknowledged])?.get(0);
    if after > acknowledged && pending >= 64 {
        return Err(StoreError::DeliveryBackpressure);
    }
    Ok(tx.query_one("INSERT INTO cantongrid.delivery_receipts(subscription_id,after_delivery) VALUES($1,$2) RETURNING id",&[&id,&after])?.get(0))
}
/// A receipt is scoped to one subscription and has a shorter lifetime than its durable ACK.
fn position(tx: &mut Transaction<'_>, id: &str, receipt: &str) -> Result<i64, StoreError> {
    if !identity(receipt) {
        return Err(StoreError::SubscriptionUnavailable);
    }
    Ok(tx.query_opt("SELECT after_delivery FROM cantongrid.delivery_receipts WHERE subscription_id=$1 AND id=$2 AND created_at>=now()-interval '5 minutes'",&[&id,&receipt])?.ok_or(StoreError::SubscriptionUnavailable)?.get(0))
}

#[cfg(test)]
mod tests;
