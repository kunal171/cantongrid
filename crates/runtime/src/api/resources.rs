//! Admission precedes body extraction; database permits survive cancelled HTTP futures.
mod polls;
use super::error;
use axum::{
    extract::{Request, State},
    http::{StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

const MAX_RESPONSE: usize = 2 * 1024 * 1024;

/// Separate budgets prevent abandoned blocking tasks from borrowing replacement request slots.
#[derive(Clone)]
pub(super) struct Limits {
    requests: Arc<Semaphore>,
    database: Arc<Semaphore>,
    deadline: Duration,
    polls: polls::Polls,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            requests: Arc::new(Semaphore::new(16)),
            database: Arc::new(Semaphore::new(16)),
            deadline: Duration::from_secs(30),
            polls: polls::Polls::default(),
        }
    }
}
impl Limits {
    /// Waiting subscriptions consume a separate quota in addition to normal admission/database slots.
    pub(super) fn poll(&self, principal: &str) -> Result<polls::Permit, &'static str> {
        self.polls.acquire(principal)
    }

    /// Move the slot into the blocking closure: dropping its JoinHandle cannot stop a SQL call.
    pub(super) async fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, &'static str> {
        let permit = self
            .database
            .clone()
            .try_acquire_owned()
            .map_err(|_| "database busy")?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| "database worker failed")
    }
}

/// Run outside the extractors, so slow bodies consume a bounded slot and have a deadline too.
pub(super) async fn admit(State(limits): State<Limits>, request: Request, next: Next) -> Response {
    let Ok(_permit) = limits.requests.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
    match tokio::time::timeout(limits.deadline, next.run(request)).await {
        Ok(response) => response,
        Err(_) => error(StatusCode::GATEWAY_TIMEOUT),
    }
}

/// A bounded writer stops serialization before allocating an arbitrarily large response.
/// For example, two valid 1.1 MiB resolver pages still exceed one 2 MiB operation budget.
struct Output(Vec<u8>);
impl io::Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RESPONSE - self.0.len() {
            return Err(io::Error::other("response exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Apply the same whole-response limit to REST and both GraphQL endpoints, including errors.
fn json(value: &impl serde::Serialize) -> Response {
    let mut output = Output(Vec::new());
    if serde_json::to_writer(&mut output, value).is_err() {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    ([(CONTENT_TYPE, "application/json")], output.0).into_response()
}

/// Encoding consumes time too. Discard the entire body if grants or a source lease change.
/// Both checks are synchronous: no response body escapes before the final authorization check.
pub(super) fn guarded_json(
    value: &impl serde::Serialize,
    authorized: impl Fn() -> bool,
) -> Response {
    if !authorized() {
        return error(StatusCode::UNAUTHORIZED);
    }
    let response = json(value);
    if !authorized() {
        return error(StatusCode::UNAUTHORIZED);
    }
    response
}

#[cfg(test)]
mod tests;

/// Static operator-visible limits describe configured budgets without exposing another caller's activity.
pub(super) fn document() -> serde_json::Value {
    let limits = Limits::default();
    serde_json::json!({"requestSlots":limits.requests.available_permits(),"databaseSlots":limits.database.available_permits(),"requestDeadlineSeconds":limits.deadline.as_secs(),"maxResponseBytes":MAX_RESPONSE,"longPollSlots":polls::MAX_POLLS,"longPollSlotsPerPrincipal":polls::PER_PRINCIPAL})
}
