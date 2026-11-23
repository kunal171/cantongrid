//! Durable pull subscriptions use the same caller/source guards as ordinary API reads.
mod bootstrap;
mod rest;
mod service;
mod types;
use super::*;
pub(super) use bootstrap::{bootstrap, snapshot};
use cantongrid_storage::{
    Consumer, DeliveryPage, DeliveryQuery, DeliveryStart, Subscription, SubscriptionRequest,
};
pub(super) use rest::{acknowledge, cancel, events, subscribe};
pub use types::{DeliveryEvent, DeliveryEvidence, DeliveryKind};

/// No input can supply party grants, an internal delivery sequence or a model processing expression.
enum Operation {
    Bootstrap {
        name: String,
    },
    Snapshot {
        id: String,
        cursor: Option<String>,
        limit: usize,
    },
    Register {
        name: String,
        start: DeliveryStart,
    },
    Read {
        id: String,
        cursor: Option<String>,
        limit: usize,
    },
    Acknowledge {
        id: String,
        cursor: String,
    },
    Cancel {
        id: String,
    },
}
/// Keep source authority alive until serialization and the final caller-policy check complete.
struct Reply {
    data: serde_json::Value,
    project: Project,
    source: SourceAuthorization,
    binding: String,
}
impl Reply {
    /// Whole-response encoding is bounded and withheld after either authority expires or changes.
    fn response(&self, token: &str) -> Response {
        resources::guarded_json(&self.data, || {
            source_valid(&self.source, &self.project)
                && auth::authenticate(&self.project, token)
                    .is_ok_and(|id| id.binding == self.binding)
        })
    }
}
/// Finite HTTP failure classes deliberately omit source/SQL details and private consumer positions.
struct Failure {
    status: StatusCode,
    coverage: Option<&'static str>,
}
impl Failure {
    fn new(status: StatusCode) -> Self {
        Self {
            status,
            coverage: None,
        }
    }
    fn storage(error: cantongrid_storage::StoreError) -> Self {
        use cantongrid_storage::StoreError;
        match error {
            StoreError::SubscriptionUnavailable => Self::new(StatusCode::NOT_FOUND),
            StoreError::BootstrapConflict => Self::new(StatusCode::CONFLICT),
            StoreError::TemporalUnavailable => Self {
                status: StatusCode::CONFLICT,
                coverage: Some("unavailable"),
            },
            StoreError::ModelQuery => Self::new(StatusCode::BAD_REQUEST),
            StoreError::DeliveryBackpressure => Self::new(StatusCode::TOO_MANY_REQUESTS),
            StoreError::DeliveryGap => Self {
                status: StatusCode::CONFLICT,
                coverage: Some("gap"),
            },
            _ => Self::new(StatusCode::SERVICE_UNAVAILABLE),
        }
    }
    fn response(self) -> Response {
        if let Some(coverage) = self.coverage {
            (self.status,Json(serde_json::json!({"error":"requested authorized coverage unavailable; explicit resynchronization may be required","coverage":coverage}))).into_response()
        } else {
            error(self.status)
        }
    }
}
/// The blocking storage layer returns random receipt IDs; only the API signs them for transport.
enum Stored {
    Bootstrapped(Subscription),
    Snapshot(cantongrid_storage::ReadPage),
    Registered(Subscription),
    Page(DeliveryPage),
    Done,
}
