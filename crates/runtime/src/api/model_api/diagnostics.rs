//! Typed projection diagnostics reuse model catalog and final source/caller/binding guards.
use super::graphql_schema::scalar;
use super::*;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, SchemaBuilder, TypeRef,
};
use axum::extract::Path;
use serde::Deserialize;
use std::sync::atomic::Ordering;

/// A visible contract root only; clients cannot upload a substitute canonical payload through the API.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    contract_id: String,
}
/// Authenticate before catalog discovery and root diagnosis, then use the existing final model guard.
pub(in crate::api) async fn rest(
    State(service): State<Service>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
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
    let Ok(bindings) = catalog(&service, &token).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Some(binding) = bindings
        .iter()
        .find(|b| b.namespace == namespace && b.model.name == name)
    else {
        return error(StatusCode::NOT_FOUND);
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
        1,
        None,
        QueryMode::Diagnostic {
            root: input.contract_id,
        },
    )
    .await
    {
        Ok(result) => finish(&service, &token, &identity.binding, &bindings, &result).await,
        Err(QueryError::Input) => error(StatusCode::BAD_REQUEST),
        Err(QueryError::NotFound) => error(StatusCode::NOT_FOUND),
        Err(QueryError::Forbidden) => error(StatusCode::FORBIDDEN),
        Err(QueryError::Unavailable) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Shared output definitions contain no values, SQL, field paths or private checkpoint keys.
pub(super) fn types(mut builder: SchemaBuilder) -> SchemaBuilder {
    let mut diagnostic = Object::new("CgProjectionDiagnostic");
    for field in cantongrid_models::ModelDiagnostic::OUTPUT_FIELDS {
        diagnostic = diagnostic.field(scalar(field, TypeRef::named_nn("String")));
    }
    diagnostic = diagnostic
        .field(scalar("active", TypeRef::named_nn("Boolean")))
        .field(scalar("errorCode", TypeRef::named("String")));
    let page = Object::new("CgProjectionPage")
        .field(scalar("visibleTotal", TypeRef::named_nn("String")))
        .field(scalar("fingerprint", TypeRef::named_nn("String")))
        .field(scalar("nextCursor", TypeRef::named("String")))
        .field(Field::new(
            "items",
            TypeRef::named_nn_list_nn("CgProjectionDiagnostic"),
            |ctx| {
                FieldFuture::new(async move {
                    let value = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                    let items = value["items"]
                        .as_array()
                        .ok_or_else(|| async_graphql::Error::new("diagnostic unavailable"))?;
                    Ok(Some(FieldValue::list(
                        items.iter().cloned().map(FieldValue::owned_any),
                    )))
                })
            },
        ));
    builder = builder.register(diagnostic).register(page);
    builder
}

/// Every diagnostic counts toward the existing two-resolver budget and inherits operation-wide guards.
pub(super) fn root(
    root: Object,
    model: &cantongrid_models::Model,
    bindings: &[BoundModel],
) -> Result<Object, String> {
    let choices: Vec<_> = bindings
        .iter()
        .filter(|b| b.model == *model)
        .cloned()
        .collect();
    Ok(root.field(
        Field::new(
            model
                .diagnostic_api_name()
                .map_err(|_| "diagnostic schema unavailable")?,
            TypeRef::named_nn("CgProjectionPage"),
            move |ctx| {
                let choices = choices.clone();
                FieldFuture::new(async move {
                    let namespace = ctx.args.try_get("namespace")?.string()?;
                    let binding = choices
                        .into_iter()
                        .find(|b| b.namespace == namespace)
                        .ok_or_else(|| async_graphql::Error::new("diagnostic unavailable"))?;
                    let root = ctx.args.try_get("contractId")?.string()?.to_owned();
                    let operation = ctx.data::<ModelRequest>()?;
                    operation
                        .remaining
                        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                        .map_err(|_| async_graphql::Error::new("model query limit exceeded"))?;
                    let request = &operation.request;
                    let value = query_mode(
                        &request.service,
                        &request.token,
                        binding,
                        1,
                        None,
                        QueryMode::Diagnostic { root },
                    )
                    .await
                    .map_err(|_| async_graphql::Error::new("diagnostic unavailable"))?;
                    Ok(Some(FieldValue::owned_any(value)))
                })
            },
        )
        .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
        .argument(InputValue::new("contractId", TypeRef::named_nn("String"))),
    ))
}
