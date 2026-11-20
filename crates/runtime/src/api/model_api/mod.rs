//! Per-request generated model schema, authenticated before metadata discovery or row resolution.
use super::{Project, RequestContext, Service, auth, bearer, current_source, error, source_valid};
mod graphql_schema;
mod rest;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use cantongrid_models::QueryOptions;
use cantongrid_storage::{BoundModel, Store};
use graphql_schema::schema;
pub(super) use rest::{aggregates, describe, list, rows};
mod aggregate_schema;
use std::sync::atomic::AtomicUsize;

/// At most two database row resolvers per operation, including aliases and fragments.
struct ModelRequest {
    request: RequestContext,
    remaining: AtomicUsize,
}

/// Discover declarations only after independent caller authentication and current source checks.
async fn catalog(service: &Service, token: &str) -> Result<Vec<BoundModel>, String> {
    let project = Project::load(&service.path)?;
    let id = auth::authenticate(&project, token)?;
    let source = current_source(&project).await?;
    service
        .limits
        .blocking(move || {
            Store::open_env(&project.destination_database_env)
                .map_err(|e| e.to_string())?
                .bound_models(
                    &project.plan,
                    &id.caller,
                    source.lease().ok_or("source unavailable")?,
                )
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "model discovery failed")?
}

/// Input errors are safe to report as 400; source/storage/authorization races stay unavailable.
enum QueryError {
    Input,
    Forbidden,
    Unavailable,
}

/// The operation domain is server-selected; aggregate input predicates are not accepted.
enum QueryMode {
    Rows(QueryOptions),
    Aggregate,
}

/// Existing row resolvers retain their filter/sort contract and cursor domain.
async fn query(
    service: &Service,
    token: &str,
    binding: BoundModel,
    limit: usize,
    cursor: Option<String>,
    options: QueryOptions,
) -> Result<serde_json::Value, QueryError> {
    query_mode(
        service,
        token,
        binding,
        limit,
        cursor,
        QueryMode::Rows(options),
    )
    .await
}

/// Aggregate resolvers share admission, current grants, signed cursors and final identity checks.
async fn aggregate_query(
    service: &Service,
    token: &str,
    binding: BoundModel,
    limit: usize,
    cursor: Option<String>,
) -> Result<serde_json::Value, QueryError> {
    query_mode(service, token, binding, limit, cursor, QueryMode::Aggregate).await
}

/// Cursor kinds include namespace and immutable revision fingerprint; promotion invalidates old pages.
async fn query_mode(
    service: &Service,
    token: &str,
    binding: BoundModel,
    limit: usize,
    cursor: Option<String>,
    mode: QueryMode,
) -> Result<serde_json::Value, QueryError> {
    if !(1..=25).contains(&limit) {
        return Err(QueryError::Input);
    }
    let project = Project::load(&service.path).map_err(|_| QueryError::Unavailable)?;
    let identity = auth::authenticate(&project, token).map_err(|_| QueryError::Unavailable)?;
    let kind = match &mode {
        QueryMode::Rows(options) => {
            let validated = binding
                .model
                .validate_query(options)
                .map_err(|_| QueryError::Input)?;
            format!(
                "model:v2:{}:{}:{}",
                binding.namespace,
                binding.fingerprint,
                validated.fingerprint()
            )
        }
        QueryMode::Aggregate => {
            binding
                .model
                .aggregate_fields()
                .map_err(|_| QueryError::Input)?;
            format!("aggregate:v1:{}:{}", binding.namespace, binding.fingerprint)
        }
    };
    let after = cursor
        .as_deref()
        .map(|s| auth::after(&project, &identity, &kind, s).map_err(|_| QueryError::Input))
        .transpose()?;
    let source = current_source(&project)
        .await
        .map_err(|_| QueryError::Unavailable)?;
    if matches!(mode, QueryMode::Aggregate) {
        let full_scope = source
            .lease()
            .and_then(|lease| {
                lease.read_parties(
                    &project.plan.context().binding(),
                    &identity.caller,
                    std::time::Instant::now(),
                )
            })
            .is_some_and(|parties| parties.len() == project.plan.source_parties().count());
        if !full_scope {
            return Err(QueryError::Forbidden);
        }
    }
    let path = service.path.clone();
    let old_identity = identity.binding.clone();
    let mut page = service
        .limits
        .blocking(move || {
            let mut store = Store::open_env(&project.destination_database_env)
                .map_err(|_| QueryError::Unavailable)?;
            let lease = source.lease().ok_or(QueryError::Unavailable)?;
            let result = match mode {
                QueryMode::Rows(options) => store.read_model_query(
                    &project.plan,
                    &identity.caller,
                    lease,
                    &binding,
                    &options,
                    after.as_deref(),
                    limit,
                ),
                QueryMode::Aggregate => store.read_aggregate(
                    &project.plan,
                    &identity.caller,
                    lease,
                    &binding,
                    after.as_deref(),
                    limit,
                ),
            };
            result.map_err(|e| {
                if e == cantongrid_storage::StoreError::ModelQuery {
                    QueryError::Input
                } else {
                    QueryError::Unavailable
                }
            })
        })
        .await
        .map_err(|_| QueryError::Unavailable)??;
    let project = Project::load(&path).map_err(|_| QueryError::Unavailable)?;
    let current = auth::authenticate(&project, token).map_err(|_| QueryError::Unavailable)?;
    if old_identity != current.binding {
        return Err(QueryError::Unavailable);
    }
    let next = page
        .next_after
        .take()
        .map(|after| {
            auth::cursor(&project, &current, &kind, after).map_err(|_| QueryError::Unavailable)
        })
        .transpose()?;
    Ok(
        serde_json::json!({"items":page.items,"visibleTotal":page.visible_total,"nextCursor":next,"fingerprint":page.fingerprint}),
    )
}

/// Reject the entire operation if authorization or any active revision changes during resolution.
/// No successful sibling resolver may escape alongside a later revocation/promotion failure.
pub(super) async fn graphql(
    State(service): State<Service>,
    headers: HeaderMap,
    body: Bytes,
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
    let request = std::str::from_utf8(&body)
        .ok()
        .and_then(|s| cantongrid_core::document::parse::<async_graphql::Request>(s).ok());
    let Some(request) = request else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Ok(bindings) = catalog(&service, &token).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(schema) = schema(&bindings) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut response = schema
        .execute(request.data(ModelRequest {
            request: RequestContext {
                service: service.clone(),
                token: token.clone(),
            },
            remaining: AtomicUsize::new(2),
        }))
        .await;
    // Dynamic execution may retain successful siblings after another resolver fails.
    // A model operation returns no data on any error, including admission and overflow.
    if !response.errors.is_empty() {
        response.data = async_graphql::Value::Null;
    }
    finish(&service, &token, &identity.binding, &bindings, &response).await
}

/// Recheck the catalog, source and caller after building a response, for every transport.
async fn finish<T: serde::Serialize>(
    service: &Service,
    token: &str,
    identity_binding: &str,
    bindings: &[BoundModel],
    value: &T,
) -> Response {
    let Ok(current_bindings) = catalog(service, token).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let identities = |bindings: &[BoundModel]| {
        bindings
            .iter()
            .map(|b| (b.namespace.clone(), b.fingerprint.clone()))
            .collect::<Vec<_>>()
    };
    if identities(bindings) != identities(&current_bindings) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Ok(latest) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(source) = current_source(&latest).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    super::resources::guarded_json(value, || {
        source_valid(&source, &latest)
            && auth::authenticate(&latest, token).is_ok_and(|id| id.binding == identity_binding)
    })
}
