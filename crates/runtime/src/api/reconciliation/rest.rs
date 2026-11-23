//! REST accepts only a configured alias and an empty object; source grants/formulas are never inputs.
use super::{Service, error, evaluate, finish, prepare};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
/// List identities only after complete source/caller scope and current model-pin verification.
pub(in crate::api) async fn list(State(service): State<Service>, headers: HeaderMap) -> Response {
    let token = match super::super::bearer(&headers) {
        Ok(token) => token,
        Err(status) => return error(status),
    };
    let prepared = match prepare(&service, &token).await {
        Ok(p) => p,
        Err(e) => return error(e.status()),
    };
    let pins = prepared
        .catalog
        .entries
        .iter()
        .map(|entry| &entry.pin)
        .collect::<Vec<_>>();
    finish(
        &service,
        &token,
        &prepared,
        &serde_json::json!({"policies":pins}),
    )
    .await
}
/// Authenticate before body/policy validation so an incomplete caller cannot probe configured aliases.
pub(in crate::api) async fn query(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(name): Path<String>,
    body: Bytes,
) -> Response {
    let token = match super::super::bearer(&headers) {
        Ok(token) => token,
        Err(status) => return error(status),
    };
    let prepared = match prepare(&service, &token).await {
        Ok(p) => p,
        Err(e) => return error(e.status()),
    };
    // Serde can represent an empty struct as []; explicitly require an empty JSON object instead.
    if !std::str::from_utf8(&body)
        .ok()
        .and_then(|s| cantongrid_core::document::parse::<serde_json::Value>(s).ok())
        .is_some_and(|value| value.as_object().is_some_and(|object| object.is_empty()))
    {
        return error(StatusCode::BAD_REQUEST);
    }
    let value = match evaluate(&service, &token, &prepared, &name).await {
        Ok(value) => value,
        Err(e) => return error(e.status()),
    };
    finish(&service, &token, &prepared, &value).await
}
