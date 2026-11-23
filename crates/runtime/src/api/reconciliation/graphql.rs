//! Typed reconciliation transport shares the REST service and discards the entire operation on error.
use super::{Prepared, Service, error, evaluate, finish, prepare};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::Deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// All exact counts and definition revisions remain strings in both public transports.
#[derive(Deserialize, SimpleObject)]
struct Identity {
    name: String,
    revision: String,
    fingerprint: String,
}
/// Namespace identifies the configured organization binding, never source witnesses or physical tables.
#[derive(Deserialize, SimpleObject)]
struct ModelIdentity {
    namespace: String,
    name: String,
    revision: String,
    fingerprint: String,
}
/// Per-check totals are finite diagnostics; no financial value or contract identifier is returned.
#[derive(Deserialize, SimpleObject)]
struct Check {
    name: String,
    satisfied: String,
    violated: String,
    unavailable: String,
}
/// Convert only a trusted storage result; HTTP never accepts this output structure as evidence.
#[derive(Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Report {
    format_version: u32,
    policy: Identity,
    model: ModelIdentity,
    definition_fingerprint: String,
    status: String,
    comparison: String,
    coverage: String,
    complete_within_configured_scope: bool,
    contracts: String,
    checks: Vec<Check>,
}
/// Configured discovery metadata omits paths, expressions and literal constants.
#[derive(Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Metadata {
    name: String,
    namespace: String,
    model: String,
    model_revision: String,
    model_fingerprint: String,
    policy_name: String,
    policy_revision: String,
    policy_fingerprint: String,
}
/// One immutable operation context and two-root budget, including aliases/fragments and discovery.
struct Request {
    service: Service,
    token: String,
    prepared: Arc<Prepared>,
    remaining: AtomicUsize,
}
impl Request {
    /// Charge before resolver work; repeated aliases cannot bypass per-operation admission.
    fn charge(&self) -> async_graphql::Result<()> {
        self.remaining
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .map(|_| ())
            .map_err(|_| async_graphql::Error::new("reconciliation unavailable"))
    }
}
/// Static fields reveal no configured policy names through schema construction or enum values.
struct Query;
#[Object]
impl Query {
    /// Discover only the prepared complete-scope catalog; final response checks still apply.
    #[graphql(complexity = 50)]
    async fn reconciliation_policies(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<Metadata>> {
        let request = ctx.data_unchecked::<Request>();
        request.charge()?;
        request
            .prepared
            .catalog
            .entries
            .iter()
            .map(|entry| {
                serde_json::to_value(&entry.pin)
                    .and_then(serde_json::from_value)
                    .map_err(|_| async_graphql::Error::new("reconciliation unavailable"))
            })
            .collect()
    }
    /// Names select operator configuration; no request field can define arithmetic or party permissions.
    #[graphql(complexity = 50)]
    async fn reconciliation(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> async_graphql::Result<Report> {
        let request = ctx.data_unchecked::<Request>();
        request.charge()?;
        let value = evaluate(&request.service, &request.token, &request.prepared, &name)
            .await
            .map_err(|_| async_graphql::Error::new("reconciliation unavailable"))?;
        serde_json::from_value(value)
            .map_err(|_| async_graphql::Error::new("reconciliation unavailable"))
    }
}
/// Authenticate/full-scope-check before parsing input; every resolver and final guard shares one snapshot context.
pub(in crate::api) async fn handle(
    State(service): State<Service>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let token = match super::super::bearer(&headers) {
        Ok(token) => token,
        Err(status) => return error(status),
    };
    let prepared = match prepare(&service, &token).await {
        Ok(p) => Arc::new(p),
        Err(e) => return error(e.status()),
    };
    let request = std::str::from_utf8(&body)
        .ok()
        .and_then(|s| cantongrid_core::document::parse::<async_graphql::Request>(s).ok());
    let Some(request) = request else {
        return error(StatusCode::BAD_REQUEST);
    };
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .limit_depth(8)
        .limit_complexity(100)
        .disable_introspection()
        .finish();
    let mut response = schema
        .execute(request.data(Request {
            service: service.clone(),
            token: token.clone(),
            prepared: prepared.clone(),
            remaining: AtomicUsize::new(2),
        }))
        .await;
    if !response.errors.is_empty() {
        response.data = async_graphql::Value::Null;
    }
    finish(&service, &token, &prepared, &response).await
}
