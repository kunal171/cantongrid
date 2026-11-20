//! Generated aggregate fields use the row API's admission budget and final delivery guards.
use super::{
    ModelRequest, aggregate_query,
    graphql_schema::{field_type, scalar},
};
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, SchemaBuilder, TypeRef,
};
use cantongrid_models::Model;
use cantongrid_storage::BoundModel;
use std::sync::atomic::Ordering;

/// Build only validated group/metric output fields; grouping and measures are never request inputs.
pub(super) fn register(
    mut builder: SchemaBuilder,
    mut root: Object,
    model: &Model,
    choices: Vec<BoundModel>,
) -> Result<(SchemaBuilder, Object), String> {
    let stem = format!("{}Aggregate", model.api_type().map_err(|e| e.to_string())?);
    let values_name = format!("{stem}Values");
    let row_name = format!("{stem}Row");
    let page_name = format!("{stem}Page");
    let mut values = Object::new(&values_name);
    for (name, kind) in model.aggregate_fields().map_err(|e| e.to_string())? {
        values = values.field(scalar(&name, field_type(&kind)));
    }
    let row = Object::new(&row_name)
        .field(scalar("groupKey", TypeRef::named_nn("String")))
        .field(Field::new(
            "values",
            TypeRef::named_nn(values_name),
            |ctx| {
                FieldFuture::new(async move {
                    Ok(Some(FieldValue::owned_any(
                        ctx.parent_value.try_downcast_ref::<serde_json::Value>()?["values"].clone(),
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
            TypeRef::named_nn_list_nn(row_name),
            |ctx| {
                FieldFuture::new(async move {
                    let parent = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                    let items = parent["items"]
                        .as_array()
                        .ok_or_else(|| async_graphql::Error::new("aggregate unavailable"))?;
                    Ok(Some(FieldValue::list(
                        items.iter().cloned().map(FieldValue::owned_any),
                    )))
                })
            },
        ));
    root = root.field(
        Field::new(
            model.aggregate_api_name().map_err(|e| e.to_string())?,
            TypeRef::named_nn(page_name),
            move |ctx| {
                let choices = choices.clone();
                FieldFuture::new(async move {
                    let namespace = ctx.args.try_get("namespace")?.string()?;
                    let binding = choices
                        .into_iter()
                        .find(|b| b.namespace == namespace)
                        .ok_or_else(|| async_graphql::Error::new("aggregate unavailable"))?;
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
                    let page = aggregate_query(
                        &request.service,
                        &request.token,
                        binding,
                        usize::try_from(limit).unwrap_or(0),
                        cursor,
                    )
                    .await
                    .map_err(|_| async_graphql::Error::new("aggregate unavailable"))?;
                    Ok(Some(FieldValue::owned_any(page)))
                })
            },
        )
        .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
        .argument(InputValue::new("limit", TypeRef::named("Int")))
        .argument(InputValue::new("cursor", TypeRef::named("String"))),
    );
    builder = builder.register(values).register(row).register(page);
    Ok((builder, root))
}
