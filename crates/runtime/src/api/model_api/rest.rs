//! REST model discovery and queries share the GraphQL catalog, row service and final guards.
use super::{Project, Service, auth, bearer, catalog, error, finish, query};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use cantongrid_models::{Comparison, QueryField, QueryOptions, QueryValue, Sorts};
use cantongrid_storage::BoundModel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Public organization catalog; definitions, literals and SQL names are deliberately absent.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Metadata {
    namespace: String,
    name: String,
    revision: String,
    fingerprint: String,
    query_field: String,
    diagnostic_query_field: String,
    fields: Vec<QueryField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relationship_query_fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    join_query_fields: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregate_query_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregate_fields: Option<Vec<QueryField>>,
}
/// Expose field/type metadata for an active binding without serializing its processing expressions.
pub(super) fn metadata(binding: &BoundModel, bindings: &[BoundModel]) -> Result<Metadata, String> {
    Ok(Metadata {
        namespace: binding.namespace.clone(),
        name: binding.model.name.clone(),
        revision: binding.model.revision.to_string(),
        fingerprint: binding.fingerprint.clone(),
        query_field: binding.model.api_name().map_err(|e| e.to_string())?,
        diagnostic_query_field: binding
            .model
            .diagnostic_api_name()
            .map_err(|_| "diagnostic metadata unavailable")?,
        fields: binding.model.query_fields().map_err(|e| e.to_string())?,
        relationship_query_fields: if binding.model.relationships.is_empty() {
            None
        } else {
            Some(
                binding
                    .model
                    .relationships
                    .iter()
                    .map(|r| {
                        binding
                            .model
                            .relationship_api_name(&r.name)
                            .map_err(|e| e.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        },
        join_query_fields: {
            let names: Vec<_> = binding
                .model
                .relationships
                .iter()
                .filter(|r| r.join.is_some())
                .filter_map(|r| {
                    super::relationships::target(bindings, binding, &r.name)
                        .ok()
                        .and_then(|target| binding.model.join_api_name(&r.name, &target.model).ok())
                })
                .collect();
            if names.is_empty() { None } else { Some(names) }
        },
        aggregate_query_field: binding
            .model
            .aggregation
            .as_ref()
            .map(|_| binding.model.aggregate_api_name())
            .transpose()
            .map_err(|e| e.to_string())?,
        aggregate_fields: binding
            .model
            .aggregation
            .as_ref()
            .map(|_| binding.model.aggregate_query_fields())
            .transpose()
            .map_err(|e| e.to_string())?,
    })
}

/// Fixed body contract keeps the same filters/sort vocabulary as generated GraphQL inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(default = "default_limit")]
    limit: usize,
    cursor: Option<String>,
    #[serde(default)]
    filters: BTreeMap<String, BTreeMap<Comparison, QueryValue>>,
    sort: Option<Sorts>,
}
/// Aggregate queries never accept contribution filters or request-supplied grouping fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateInput {
    #[serde(default = "default_limit")]
    limit: usize,
    cursor: Option<String>,
}

/// Query whole authorized groups through the same delivery guard as row reads.
pub(in crate::api) async fn aggregates(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(path): Path<(String, String)>,
    body: Bytes,
) -> Response {
    handle(service, headers, Some(path), Some(body), true).await
}

/// Match the default page size used by generic queries and generated GraphQL model roots.
fn default_limit() -> usize {
    20
}

/// List the active organization catalog only after caller and source checks.
pub(in crate::api) async fn list(State(service): State<Service>, headers: HeaderMap) -> Response {
    handle(service, headers, None, None, false).await
}
/// Describe one active namespace/model through the same guarded catalog as list queries.
pub(in crate::api) async fn describe(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(path): Path<(String, String)>,
) -> Response {
    handle(service, headers, Some(path), None, false).await
}
/// Pass a bounded model body to shared validation, scoped SQL and final delivery checks.
pub(in crate::api) async fn rows(
    State(service): State<Service>,
    headers: HeaderMap,
    Path(path): Path<(String, String)>,
    body: Bytes,
) -> Response {
    handle(service, headers, Some(path), Some(body), false).await
}

/// Authenticate before revealing catalog existence or validating model-specific input fields.
async fn handle(
    service: Service,
    headers: HeaderMap,
    path: Option<(String, String)>,
    body: Option<Bytes>,
    aggregate_mode: bool,
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
    let value = if let Some((namespace, name)) = path {
        let Some(binding) = bindings
            .iter()
            .find(|b| b.namespace == namespace && b.model.name == name)
        else {
            return error(StatusCode::NOT_FOUND);
        };
        if let Some(body) = body {
            // Strict decoding rejects duplicate keys and floats before serde can lose that evidence.
            let input = std::str::from_utf8(&body).ok().and_then(|text| {
                if aggregate_mode {
                    cantongrid_core::document::parse::<AggregateInput>(text)
                        .ok()
                        .map(|input| Input {
                            limit: input.limit,
                            cursor: input.cursor,
                            filters: BTreeMap::new(),
                            sort: None,
                        })
                } else {
                    cantongrid_core::document::parse::<Input>(text).ok()
                }
            });
            if aggregate_mode && binding.model.aggregation.is_none() {
                return error(StatusCode::BAD_REQUEST);
            }
            let Some(input) = input else {
                return error(StatusCode::BAD_REQUEST);
            };
            let options = QueryOptions {
                filters: input.filters,
                sort: input.sort,
            };
            if !(1..=25).contains(&input.limit) || binding.model.validate_query(&options).is_err() {
                return error(StatusCode::BAD_REQUEST);
            }
            // Invalid or expired cursors fail closed; no fallback to an unfiltered first page.
            let result = if aggregate_mode {
                super::aggregate_query(&service, &token, binding.clone(), input.limit, input.cursor)
                    .await
            } else {
                query(
                    &service,
                    &token,
                    binding.clone(),
                    input.limit,
                    input.cursor,
                    options,
                )
                .await
            };
            match result {
                Ok(page) => page,
                Err(super::QueryError::NotFound) => return error(StatusCode::NOT_FOUND),
                Err(super::QueryError::Input) => return error(StatusCode::BAD_REQUEST),
                Err(super::QueryError::Forbidden) => return error(StatusCode::FORBIDDEN),
                Err(super::QueryError::Unavailable) => {
                    return error(StatusCode::SERVICE_UNAVAILABLE);
                }
            }
        } else {
            let Ok(metadata) = metadata(binding, &bindings)
                .and_then(|m| serde_json::to_value(m).map_err(|_| "metadata unavailable".into()))
            else {
                return error(StatusCode::SERVICE_UNAVAILABLE);
            };
            metadata
        }
    } else {
        let metadata = bindings
            .iter()
            .map(|b| metadata(b, &bindings))
            .collect::<Result<Vec<_>, _>>();
        let Ok(value) = metadata
            .and_then(|m| serde_json::to_value(m).map_err(|_| "metadata unavailable".into()))
        else {
            return error(StatusCode::SERVICE_UNAVAILABLE);
        };
        serde_json::json!({"models":value})
    };
    finish(&service, &token, &identity.binding, &bindings, &value).await
}
