//! Generated typed inner-join roots share admission and final authorization with model queries.
use super::{
    BoundModel, ModelRequest, QueryMode,
    graphql_schema::{field_type, scalar},
    query_mode, relationships,
};
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, SchemaBuilder, TypeRef,
};
use cantongrid_models::Model;
use std::{collections::BTreeMap, sync::atomic::Ordering};

/// Resolve compatible active target pairs only; stale declarations never widen to another revision.
pub(super) fn register(
    mut builder: SchemaBuilder,
    mut root: Object,
    model: &Model,
    name: &str,
    bindings: &[BoundModel],
) -> Result<(SchemaBuilder, Object), String> {
    let mut groups: BTreeMap<String, Vec<(BoundModel, BoundModel)>> = BTreeMap::new();
    for source in bindings.iter().filter(|b| b.model == *model) {
        if let Ok(target) = relationships::target(bindings, source, name)
            && model.join_fields(name, &target.model).is_ok()
        {
            groups
                .entry(target.fingerprint.clone())
                .or_default()
                .push((source.clone(), target));
        }
    }
    for choices in groups.into_values() {
        let target = &choices[0].1.model;
        let stem = model
            .join_api_type(name, target)
            .map_err(|e| e.to_string())?;
        let (source_fields, target_fields) =
            model.join_fields(name, target).map_err(|e| e.to_string())?;
        for (suffix, fields) in [
            ("SourceValues", source_fields),
            ("TargetValues", target_fields),
        ] {
            let mut values = Object::new(format!("{stem}{suffix}"));
            for (field, kind) in fields {
                values = values.field(scalar(&field, field_type(&kind)));
            }
            builder = builder.register(values);
        }
        let row_name = format!("{stem}Row");
        let page_name = format!("{stem}Page");
        let mut row = Object::new(&row_name);
        for key in cantongrid_models::Relationship::OUTPUT_FIELDS {
            row = row.field(scalar(key, TypeRef::named_nn("String")));
        }
        row = row
            .field(scalar("depth", TypeRef::named_nn("Int")))
            .field(scalar("joinKind", TypeRef::named_nn("String")));
        for (key, suffix) in [
            ("sourceValues", "SourceValues"),
            ("targetValues", "TargetValues"),
        ] {
            row = row.field(Field::new(
                key,
                TypeRef::named_nn(format!("{stem}{suffix}")),
                move |ctx| {
                    FieldFuture::new(async move {
                        Ok(Some(FieldValue::owned_any(
                            ctx.parent_value.try_downcast_ref::<serde_json::Value>()?[key].clone(),
                        )))
                    })
                },
            ));
        }
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
                            .ok_or_else(|| async_graphql::Error::new("join unavailable"))?;
                        Ok(Some(FieldValue::list(
                            items.iter().cloned().map(FieldValue::owned_any),
                        )))
                    })
                },
            ));
        let api_name = model
            .join_api_name(name, target)
            .map_err(|e| e.to_string())?;
        let name = name.to_owned();
        root = root.field(
            Field::new(api_name, TypeRef::named_nn(page_name), move |ctx| {
                let choices = choices.clone();
                let name = name.clone();
                FieldFuture::new(async move {
                    let namespace = ctx.args.try_get("namespace")?.string()?;
                    let (source, target) = choices
                        .into_iter()
                        .find(|(s, _)| s.namespace == namespace)
                        .ok_or_else(|| async_graphql::Error::new("join unavailable"))?;
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
                    let page = query_mode(
                        &request.service,
                        &request.token,
                        source,
                        usize::try_from(limit).unwrap_or(0),
                        cursor,
                        QueryMode::Relationship {
                            name,
                            root,
                            target: Box::new(target),
                            joined: true,
                        },
                    )
                    .await
                    .map_err(|_| async_graphql::Error::new("join unavailable"))?;
                    Ok(Some(FieldValue::owned_any(page)))
                })
            })
            .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
            .argument(InputValue::new("contractId", TypeRef::named_nn("String")))
            .argument(InputValue::new("limit", TypeRef::named("Int")))
            .argument(InputValue::new("cursor", TypeRef::named("String"))),
        );
        builder = builder.register(row).register(page);
    }
    Ok((builder, root))
}
