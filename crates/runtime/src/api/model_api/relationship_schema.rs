//! Generated declared-link roots return endpoint/provenance records, never joined payloads.
use super::{BoundModel, ModelRequest, graphql_schema::scalar, relationships};
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, SchemaBuilder, TypeRef,
};
use cantongrid_models::Model;
use std::sync::atomic::Ordering;

/// Register one immutable declaration with the shared two-resolver admission budget.
pub(super) fn register(
    mut builder: SchemaBuilder,
    mut root: Object,
    model: &Model,
    name: &str,
    bindings: &[BoundModel],
) -> Result<(SchemaBuilder, Object), String> {
    let stem = format!(
        "{}Relationship_{}",
        model.api_type().map_err(|e| e.to_string())?,
        name
    );
    let row_name = format!("{stem}Row");
    let page_name = format!("{stem}Page");
    let mut row = Object::new(&row_name);
    for key in cantongrid_models::Relationship::OUTPUT_FIELDS {
        row = row.field(scalar(key, TypeRef::named_nn("String")));
    }
    row = row.field(scalar("depth", TypeRef::named_nn("Int")));
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
                        .ok_or_else(|| async_graphql::Error::new("relationship unavailable"))?;
                    Ok(Some(FieldValue::list(
                        items.iter().cloned().map(FieldValue::owned_any),
                    )))
                })
            },
        ));
    let choices: Vec<_> = bindings
        .iter()
        .filter(|b| b.model == *model)
        .cloned()
        .collect();
    let all = bindings.to_vec();
    let declaration = name.to_owned();
    root = root.field(
        Field::new(
            model
                .relationship_api_name(name)
                .map_err(|e| e.to_string())?,
            TypeRef::named_nn(page_name),
            move |ctx| {
                let choices = choices.clone();
                let all = all.clone();
                let declaration = declaration.clone();
                FieldFuture::new(async move {
                    let namespace = ctx.args.try_get("namespace")?.string()?;
                    let source = choices
                        .into_iter()
                        .find(|b| b.namespace == namespace)
                        .ok_or_else(|| async_graphql::Error::new("relationship unavailable"))?;
                    let target = relationships::target(&all, &source, &declaration)
                        .map_err(|_| async_graphql::Error::new("relationship unavailable"))?;
                    let root = ctx.args.try_get("contractId")?.string()?.to_owned();
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
                    let result = relationships::query(
                        &request.service,
                        &request.token,
                        source,
                        target,
                        declaration,
                        root,
                        super::super::Filter {
                            limit: Some(usize::try_from(limit).unwrap_or(0)),
                            cursor,
                        },
                    )
                    .await
                    .map_err(|_| async_graphql::Error::new("relationship unavailable"))?;
                    Ok(Some(FieldValue::owned_any(result)))
                })
            },
        )
        .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
        .argument(InputValue::new("contractId", TypeRef::named_nn("String")))
        .argument(InputValue::new("limit", TypeRef::named("Int")))
        .argument(InputValue::new("cursor", TypeRef::named("String"))),
    );
    builder = builder.register(row).register(page);
    Ok((builder, root))
}
