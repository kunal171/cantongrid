//! Per-request generated model schema, authenticated before metadata discovery or row resolution.
use super::{Project, RequestContext, Service, auth, bearer, current_source, error, source_valid};
use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, Schema, TypeRef};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use cantongrid_models::FieldType;
use cantongrid_storage::{BoundModel, Store};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

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
    tokio::task::spawn_blocking(move || {
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

/// Cursor kinds include namespace and immutable revision fingerprint; promotion invalidates old pages.
async fn query(
    service: &Service,
    token: &str,
    binding: BoundModel,
    limit: usize,
    cursor: Option<String>,
) -> Result<serde_json::Value, String> {
    if !(1..=25).contains(&limit) {
        return Err("invalid page limit".into());
    }
    let project = Project::load(&service.path)?;
    let identity = auth::authenticate(&project, token)?;
    let kind = format!("model:{}:{}", binding.namespace, binding.fingerprint);
    let after = cursor
        .as_deref()
        .map(|s| auth::after(&project, &identity, &kind, s))
        .transpose()?;
    let source = current_source(&project).await?;
    let path = service.path.clone();
    let old_identity = identity.binding.clone();
    let mut page = tokio::task::spawn_blocking(move || {
        Store::open_env(&project.destination_database_env)
            .map_err(|e| e.to_string())?
            .read_model(
                &project.plan,
                &identity.caller,
                source.lease().ok_or("source unavailable")?,
                &binding,
                after.as_deref(),
                limit,
            )
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "model query failed")??;
    let project = Project::load(&path)?;
    let current = auth::authenticate(&project, token)?;
    if old_identity != current.binding {
        return Err("authorization changed".into());
    }
    let next = page
        .next_after
        .take()
        .map(|after| auth::cursor(&project, &current, &kind, after))
        .transpose()?;
    Ok(
        serde_json::json!({"items":page.items,"visibleTotal":page.visible_total,"nextCursor":next,"fingerprint":page.fingerprint}),
    )
}

/// Resolve primitive output fields from exact JSON strings/bools, without numeric conversions.
fn scalar(name: &str, kind: TypeRef) -> Field {
    let key = name.to_owned();
    Field::new(name, kind, move |ctx| {
        let key = key.clone();
        FieldFuture::new(async move {
            let row = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
            let value =
                async_graphql::Value::from_json(row.get(&key).cloned().unwrap_or_default())?;
            Ok(Some(FieldValue::value(value)))
        })
    })
}
fn field_type(kind: &FieldType) -> TypeRef {
    match kind {
        FieldType::Optional { item } => TypeRef::named(if matches!(**item, FieldType::Bool) {
            "Boolean"
        } else {
            "String"
        }),
        FieldType::Bool => TypeRef::named_nn("Boolean"),
        _ => TypeRef::named_nn("String"),
    }
}

/// All type and field names come from the compiler. Aliases share a type and select their
/// namespace explicitly; no query can name an inactive/unselected model or provide party grants.
fn schema(bindings: &[BoundModel]) -> Result<Schema, String> {
    let mut builder = Schema::build("Query", None, None)
        .limit_depth(8)
        .limit_complexity(100)
        .disable_introspection();
    let mut root = Object::new("Query");
    let unique: BTreeMap<_, _> = bindings
        .iter()
        .map(|b| (b.fingerprint.clone(), b.model.clone()))
        .collect();
    if unique.is_empty() {
        root = root.field(scalar("unavailable", TypeRef::named("String")));
    }
    for (_, model) in unique {
        let stem = model.api_type().map_err(|e| e.to_string())?;
        let values_name = format!("{stem}Values");
        let row_name = format!("{stem}Row");
        let page_name = format!("{stem}Page");
        let mut values = Object::new(&values_name);
        for (name, expression) in &model.fields {
            values = values.field(scalar(
                name,
                field_type(&expression.output_type().map_err(|e| e.to_string())?),
            ));
        }
        let row = Object::new(&row_name)
            .field(scalar("contractId", TypeRef::named_nn("String")))
            .field(Field::new(
                "values",
                TypeRef::named_nn(&values_name),
                |ctx| {
                    FieldFuture::new(async move {
                        Ok(Some(FieldValue::owned_any(
                            ctx.parent_value.try_downcast_ref::<serde_json::Value>()?["values"]
                                .clone(),
                        )))
                    })
                },
            ));
        let page = Object::new(&page_name)
            .field(scalar("visibleTotal", TypeRef::named_nn("String")))
            .field(scalar("nextCursor", TypeRef::named("String")))
            .field(scalar("fingerprint", TypeRef::named_nn("String")))
            .field(Field::new(
                "items",
                TypeRef::named_nn_list_nn(&row_name),
                |ctx| {
                    FieldFuture::new(async move {
                        let page = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                        let items = page["items"]
                            .as_array()
                            .ok_or_else(|| async_graphql::Error::new("model query unavailable"))?;
                        Ok(Some(FieldValue::list(
                            items.iter().cloned().map(FieldValue::owned_any),
                        )))
                    })
                },
            ));
        let choices: Vec<_> = bindings
            .iter()
            .filter(|b| b.model == model)
            .cloned()
            .collect();
        root = root.field(
            Field::new(
                model.api_name().map_err(|e| e.to_string())?,
                TypeRef::named_nn(page_name),
                move |ctx| {
                    let choices = choices.clone();
                    FieldFuture::new(async move {
                        let namespace = ctx.args.try_get("namespace")?.string()?;
                        let binding = choices
                            .into_iter()
                            .find(|b| b.namespace == namespace)
                            .ok_or_else(|| async_graphql::Error::new("model query unavailable"))?;
                        let limit = ctx
                            .args
                            .get("limit")
                            .filter(|v| !v.is_null())
                            .map(|v| v.i64())
                            .transpose()?
                            .unwrap_or(20);
                        let cursor = ctx
                            .args
                            .get("cursor")
                            .filter(|v| !v.is_null())
                            .map(|v| v.string().map(str::to_owned))
                            .transpose()?;
                        let operation = ctx.data::<ModelRequest>()?;
                        operation
                            .remaining
                            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                            .map_err(|_| async_graphql::Error::new("model query limit exceeded"))?;
                        let request = &operation.request;
                        let page = query(
                            &request.service,
                            &request.token,
                            binding,
                            usize::try_from(limit).unwrap_or(0),
                            cursor,
                        )
                        .await
                        .map_err(|_| async_graphql::Error::new("model query unavailable"))?;
                        Ok(Some(FieldValue::owned_any(page)))
                    })
                },
            )
            .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
            .argument(InputValue::new("limit", TypeRef::named("Int")))
            .argument(InputValue::new("cursor", TypeRef::named("String"))),
        );
        builder = builder.register(values).register(row).register(page);
    }
    builder
        .register(root)
        .finish()
        .map_err(|_| "generated schema unavailable".into())
}

/// Reject the entire operation if authorization or any active revision changes during resolution.
/// No successful sibling resolver may escape alongside a later revocation/promotion failure.
pub(super) async fn graphql(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(request): Json<async_graphql::Request>,
) -> Response {
    let Ok(_permit) = service.permits.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
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
    let Ok(schema) = schema(&bindings) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let response = schema
        .execute(request.data(ModelRequest {
            request: RequestContext {
                service: service.clone(),
                token: token.clone(),
            },
            remaining: AtomicUsize::new(2),
        }))
        .await;
    let Ok(current_bindings) = catalog(&service, &token).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let identities = |bindings: &[BoundModel]| {
        bindings
            .iter()
            .map(|b| (b.namespace.clone(), b.fingerprint.clone()))
            .collect::<Vec<_>>()
    };
    if identities(&bindings) != identities(&current_bindings) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Ok(latest) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(source) = current_source(&latest).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    if !source_valid(&source, &latest)
        || !auth::authenticate(&latest, &token).is_ok_and(|id| id.binding == identity.binding)
    {
        return error(StatusCode::UNAUTHORIZED);
    }
    let Ok(bytes) = serde_json::to_vec(&response) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    if bytes.len() > 2 * 1024 * 1024 {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let response = Json(response).into_response();
    if !source_valid(&source, &latest)
        || !auth::authenticate(&latest, &token).is_ok_and(|id| id.binding == identity.binding)
    {
        return error(StatusCode::UNAUTHORIZED);
    }
    response
}
