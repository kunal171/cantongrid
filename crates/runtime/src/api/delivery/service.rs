//! Reload policy before each bounded database operation and again before issuing data or resume tokens.
use super::*;
impl Service {
    /// A subscription never turns a previous source check into long-lived permission to read.
    pub(super) async fn delivery(
        &self,
        token: &str,
        operation: Operation,
    ) -> Result<Reply, Failure> {
        let unavailable = || Failure::new(StatusCode::SERVICE_UNAVAILABLE);
        let project = Project::load(&self.path).map_err(|_| unavailable())?;
        let identity = auth::authenticate(&project, token)
            .map_err(|_| Failure::new(StatusCode::UNAUTHORIZED))?;
        let binding = identity.binding.clone();
        let consumer = {
            use sha2::{Digest, Sha256};
            let principal = Sha256::digest(
                serde_json::json!([identity.subject, project.api.issuer, project.api.audience])
                    .to_string(),
            )
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
            Consumer::from_verified(principal, binding.clone()).map_err(Failure::storage)?
        };
        let domain = match &operation {
            Operation::Read { id, .. } | Operation::Acknowledge { id, .. } => {
                Some(format!("delivery:{id}"))
            }
            _ => None,
        };
        let after = match &operation {
            Operation::Read {
                cursor: Some(cursor),
                ..
            }
            | Operation::Acknowledge { cursor, .. } => Some(
                auth::after(
                    &project,
                    &identity,
                    domain.as_deref().ok_or_else(unavailable)?,
                    cursor,
                )
                .map_err(|_| Failure::new(StatusCode::BAD_REQUEST))?,
            ),
            _ => None,
        };
        let original_cursor = match &operation {
            Operation::Read { cursor, .. } => cursor.clone(),
            _ => None,
        };
        let source = current_source(&project).await.map_err(|_| unavailable())?;
        let stored = self
            .limits
            .blocking(move || {
                let lease = source.lease().ok_or_else(unavailable)?;
                let mut store = Store::open_env(&project.destination_database_env)
                    .map_err(|_| unavailable())?;
                match operation {
                    Operation::Register { name, start } => store
                        .subscribe(
                            &project.plan,
                            &identity.caller,
                            lease,
                            &consumer,
                            SubscriptionRequest { name: &name, start },
                        )
                        .map(Stored::Registered),
                    Operation::Read { id, limit, .. } => store
                        .read_deliveries(
                            &project.plan,
                            &identity.caller,
                            lease,
                            &consumer,
                            DeliveryQuery {
                                subscription: &id,
                                after: after.as_deref(),
                                limit,
                            },
                        )
                        .map(Stored::Page),
                    Operation::Acknowledge { id, .. } => store
                        .acknowledge(
                            &project.plan,
                            &identity.caller,
                            lease,
                            &consumer,
                            &id,
                            after.as_deref().ok_or_else(unavailable)?,
                        )
                        .map(|_| Stored::Done),
                    Operation::Cancel { id } => store
                        .cancel_subscription(&project.plan, &identity.caller, lease, &consumer, &id)
                        .map(|_| Stored::Done),
                }
                .map_err(Failure::storage)
            })
            .await
            .map_err(|_| unavailable())??;
        let project = Project::load(&self.path).map_err(|_| unavailable())?;
        let current = auth::authenticate(&project, token)
            .map_err(|_| Failure::new(StatusCode::UNAUTHORIZED))?;
        if current.binding != binding {
            return Err(Failure::new(StatusCode::UNAUTHORIZED));
        }
        let source = current_source(&project).await.map_err(|_| unavailable())?;
        let data = match stored {
            Stored::Registered(sub) => {
                let cursor = auth::cursor(
                    &project,
                    &current,
                    &format!("delivery:{}", sub.id),
                    sub.receipt,
                )
                .map_err(|_| unavailable())?;
                serde_json::json!({"id":sub.id,"cursor":cursor,"start":sub.start,"expiresAtSeconds":sub.expires_at_seconds,"formatVersion":1,"delivery":"at_least_once","coverage":"retained_outbox_only"})
            }
            Stored::Page(page) => {
                let cursor = page
                    .receipt
                    .map(|receipt| {
                        auth::cursor(
                            &project,
                            &current,
                            domain.as_deref().unwrap_or("invalid"),
                            receipt,
                        )
                    })
                    .transpose()
                    .map_err(|_| unavailable())?
                    .or(original_cursor);
                let events = page
                    .events
                    .into_iter()
                    .map(|event| {
                        serde_json::from_value::<DeliveryEvent>(public_event(event))
                            .map_err(|_| unavailable())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                serde_json::json!({"formatVersion":1,"events":events,"nextCursor":cursor,"more":page.more,"delivery":"at_least_once","coverage":"retained_authorized_outbox"})
            }
            Stored::Done => serde_json::json!({"ok":true}),
        };
        Ok(Reply {
            data,
            project,
            source,
            binding,
        })
    }
}
