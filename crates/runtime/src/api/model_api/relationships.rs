//! Declared relationship transport shares model discovery, current authority and final guards.
use super::*;
use axum::extract::Path;
use serde::Deserialize;

/// Clients select a root and page only. They cannot supply fields, target policies or party grants.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    contract_id: String,
    #[serde(default = "default_limit")]
    limit: usize,
    cursor: Option<String>,
}
/// Keep the ordinary page default across REST and generated GraphQL.
fn default_limit() -> usize {
    20
}

/// Resolve the exact active target within the source namespace; no cross-namespace widening.
pub(super) fn target(
    bindings: &[BoundModel],
    source: &BoundModel,
    name: &str,
) -> Result<BoundModel, QueryError> {
    let link = source
        .model
        .relationship(name)
        .map_err(|_| QueryError::Input)?;
    let target = bindings
        .iter()
        .find(|b| {
            b.namespace == source.namespace
                && b.model.name == link.target.model
                && b.model.revision == link.target.revision
        })
        .ok_or(QueryError::Unavailable)?;
    source
        .model
        .validate_relationship(name, &target.model)
        .map_err(|_| QueryError::Unavailable)?;
    Ok(target.clone())
}

/// Delegate to the common model operation guard and pin both endpoint fingerprints in its cursor.
pub(super) async fn query(
    service: &Service,
    token: &str,
    source: BoundModel,
    target: BoundModel,
    name: String,
    root: String,
    filter: super::super::Filter,
) -> Result<serde_json::Value, QueryError> {
    query_mode(
        service,
        token,
        source,
        filter.limit.unwrap_or(20),
        filter.cursor,
        QueryMode::Relationship {
            name,
            root,
            target: Box::new(target),
            joined: false,
        },
    )
    .await
}

/// Authenticate before model discovery; hide missing and inaccessible contract roots uniformly.
pub(in crate::api) async fn rest(
    State(service): State<Service>,
    headers: HeaderMap,
    Path((namespace, model, name)): Path<(String, String, String)>,
    body: Bytes,
) -> Response {
    handle(service, headers, (namespace, model, name), body, false).await
}

/// Join routes choose their operation on the server, never from untrusted body fields.
pub(in crate::api) async fn joined(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(path): Path<(String, String, String)>,
    body: Bytes,
) -> Response {
    handle(service, headers, path, body, true).await
}

/// Keep identical authentication, strict decoding and final catalog checks for both operations.
async fn handle(
    service: Service,
    headers: HeaderMap,
    (namespace, model, name): (String, String, String),
    body: Bytes,
    joined: bool,
) -> Response {
    let Ok(token) = bearer(&headers) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let Ok(project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(identity) = auth::authenticate(&project, &token) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let Ok(bindings) = catalog(&service, &token).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Some(binding) = bindings
        .iter()
        .find(|b| b.namespace == namespace && b.model.name == model)
    else {
        return error(StatusCode::NOT_FOUND);
    };
    if binding.model.relationship(&name).is_err() {
        return error(StatusCode::NOT_FOUND);
    }
    if joined
        && binding
            .model
            .relationship(&name)
            .is_ok_and(|r| r.join.is_none())
    {
        return error(StatusCode::NOT_FOUND);
    }
    let Ok(target) = target(&bindings, binding, &name) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Some(input) = std::str::from_utf8(&body)
        .ok()
        .and_then(|s| cantongrid_core::document::parse::<Input>(s).ok())
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    match query_mode(
        &service,
        &token,
        binding.clone(),
        input.limit,
        input.cursor,
        QueryMode::Relationship {
            name,
            root: input.contract_id,
            target: Box::new(target),
            joined,
        },
    )
    .await
    {
        Ok(page) => finish(&service, &token, &identity.binding, &bindings, &page).await,
        Err(QueryError::Input) => error(StatusCode::BAD_REQUEST),
        Err(QueryError::NotFound) => error(StatusCode::NOT_FOUND),
        Err(QueryError::Forbidden) => error(StatusCode::FORBIDDEN),
        Err(QueryError::Unavailable) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
