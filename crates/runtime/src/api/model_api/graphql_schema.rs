//! Model-specific GraphQL schema construction; service queries retain all authorization checks.
use super::{ModelRequest, query};
use async_graphql::dynamic::{
    Enum, EnumItem, Field, FieldFuture, FieldValue, InputObject, InputValue, Object, Schema,
    TypeRef,
};
use cantongrid_models::{Comparison, FieldType, QueryOptions};
use cantongrid_storage::BoundModel;
use std::{collections::BTreeMap, sync::atomic::Ordering};
/// Resolve primitive output fields from exact JSON strings/bools, without numeric conversions.
pub(super) fn scalar(name: &str, kind: TypeRef) -> Field {
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
/// Map booleans natively and exact financial scalars to strings, preserving optional nullability.
pub(super) fn field_type(kind: &FieldType) -> TypeRef {
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
pub(super) fn schema(bindings: &[BoundModel]) -> Result<Schema, String> {
    let mut builder = Schema::build("Query", None, None)
        .limit_depth(8)
        .limit_complexity(100)
        .disable_introspection();
    let mut root = Object::new("Query");
    let metadata = bindings
        .iter()
        .map(|b| super::rest::metadata(b, bindings))
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = serde_json::to_value(metadata).map_err(|_| "metadata unavailable")?;
    root = root.field(Field::new(
        "models",
        TypeRef::named_nn_list_nn("CgModel"),
        move |_| {
            let metadata = metadata.clone();
            FieldFuture::new(async move {
                Ok(Some(FieldValue::list(
                    metadata
                        .as_array()
                        .ok_or_else(|| async_graphql::Error::new("metadata unavailable"))?
                        .iter()
                        .cloned()
                        .map(FieldValue::owned_any),
                )))
            })
        },
    ));
    let model_fields = Object::new("CgModelField")
        .field(scalar("name", TypeRef::named_nn("String")))
        .field(scalar("kind", TypeRef::named_nn("String")))
        .field(scalar("nullable", TypeRef::named_nn("Boolean")))
        .field(scalar("scale", TypeRef::named("Int")))
        .field(scalar("operators", TypeRef::named_nn_list_nn("String")));
    let mut model_metadata = Object::new("CgModel");
    for name in ["namespace", "name", "revision", "fingerprint", "queryField"] {
        model_metadata = model_metadata.field(scalar(name, TypeRef::named_nn("String")));
    }
    model_metadata = model_metadata.field(Field::new(
        "relationshipQueryFields",
        TypeRef::named_nn_list("String"),
        |ctx| {
            FieldFuture::new(async move {
                let parent = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                Ok(parent
                    .get("relationshipQueryFields")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        FieldValue::list(items.iter().cloned().map(|item| {
                            FieldValue::value(
                                async_graphql::Value::from_json(item).unwrap_or_default(),
                            )
                        }))
                    }))
            })
        },
    ));
    model_metadata = model_metadata.field(Field::new(
        "joinQueryFields",
        TypeRef::named_nn_list("String"),
        |ctx| {
            FieldFuture::new(async move {
                let parent = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                Ok(parent
                    .get("joinQueryFields")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        FieldValue::list(items.iter().cloned().map(|item| {
                            FieldValue::value(
                                async_graphql::Value::from_json(item).unwrap_or_default(),
                            )
                        }))
                    }))
            })
        },
    ));
    model_metadata = model_metadata.field(scalar("aggregateQueryField", TypeRef::named("String")));
    model_metadata = model_metadata.field(Field::new(
        "aggregateFields",
        TypeRef::named_nn_list("CgModelField"),
        |ctx| {
            FieldFuture::new(async move {
                let parent = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                Ok(parent
                    .get("aggregateFields")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        FieldValue::list(items.iter().cloned().map(FieldValue::owned_any))
                    }))
            })
        },
    ));
    model_metadata = model_metadata.field(Field::new(
        "fields",
        TypeRef::named_nn_list_nn("CgModelField"),
        |ctx| {
            FieldFuture::new(async move {
                let parent = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                Ok(Some(FieldValue::list(
                    parent["fields"]
                        .as_array()
                        .ok_or_else(|| async_graphql::Error::new("metadata unavailable"))?
                        .iter()
                        .cloned()
                        .map(FieldValue::owned_any),
                )))
            })
        },
    ));
    builder = builder
        .register(model_fields)
        .register(model_metadata)
        .register(
            Enum::new("CgSortDirection")
                .item(EnumItem::new("asc"))
                .item(EnumItem::new("desc")),
        );
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
        let mut filters = InputObject::new(format!("{stem}Filter"));
        for (name, expression) in &model.fields {
            let kind = expression.output_type().map_err(|e| e.to_string())?;
            let operators_name = format!("{stem}_{name}Predicate");
            let mut operators = InputObject::new(&operators_name);
            for operator in kind.query_operators() {
                let scalar = if operator == Comparison::IsNull
                    || matches!(kind.query_scalar().0, FieldType::Bool)
                {
                    "Boolean"
                } else {
                    "String"
                };
                operators =
                    operators.field(InputValue::new(operator.name(), TypeRef::named(scalar)));
            }
            filters = filters.field(InputValue::new(name, TypeRef::named(&operators_name)));
            builder = builder.register(operators);
        }
        builder = builder.register(filters).register(
            InputObject::new(format!("{stem}Sort"))
                .field(InputValue::new("field", TypeRef::named_nn("String")))
                .field(InputValue::new(
                    "direction",
                    TypeRef::named("CgSortDirection"),
                )),
        );
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
                        let mut options = serde_json::Map::new();
                        for name in ["filters", "sort"] {
                            if let Some(value) = ctx.args.get(name).filter(|v| !v.is_null()) {
                                options.insert(name.into(), value.as_value().clone().into_json()?);
                            }
                        }
                        if let Some(value) = ctx.args.get("orderBy").filter(|v| !v.is_null()) {
                            if options.contains_key("sort") {
                                return Err(async_graphql::Error::new(
                                    "use sort or orderBy, not both",
                                ));
                            }
                            options.insert("sort".into(), value.as_value().clone().into_json()?);
                        }
                        let options: QueryOptions = serde_json::from_value(options.into())
                            .map_err(|_| async_graphql::Error::new("invalid model query"))?;
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
                            options,
                        )
                        .await
                        .map_err(|_| async_graphql::Error::new("model query unavailable"))?;
                        Ok(Some(FieldValue::owned_any(page)))
                    })
                },
            )
            .argument(InputValue::new("namespace", TypeRef::named_nn("String")))
            .argument(InputValue::new("limit", TypeRef::named("Int")))
            .argument(InputValue::new("cursor", TypeRef::named("String")))
            .argument(InputValue::new(
                "filters",
                TypeRef::named(format!("{stem}Filter")),
            ))
            .argument(InputValue::new(
                "sort",
                TypeRef::named(format!("{stem}Sort")),
            ))
            .argument(InputValue::new(
                "orderBy",
                TypeRef::named_nn_list(format!("{stem}Sort")),
            )),
        );
        builder = builder.register(values).register(row).register(page);
        for link in &model.relationships {
            (builder, root) =
                super::relationship_schema::register(builder, root, &model, &link.name, bindings)?;
            if link.join.is_some() {
                (builder, root) =
                    super::join_schema::register(builder, root, &model, &link.name, bindings)?;
            }
        }
        if model.aggregation.is_some() {
            let choices = bindings
                .iter()
                .filter(|b| b.model == model)
                .cloned()
                .collect();
            (builder, root) = super::aggregate_schema::register(builder, root, &model, choices)?;
        }
    }
    builder
        .register(root)
        .finish()
        .map_err(|_| "generated schema unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::parser::{
        parse_schema,
        types::{TypeKind, TypeSystemDefinition},
    };
    use cantongrid_models::Catalog;

    /// Compare public type/argument signatures, ignoring SDL formatting and built-in definitions.
    fn signatures(sdl: &str) -> BTreeMap<String, Vec<String>> {
        parse_schema(sdl)
            .unwrap()
            .definitions
            .into_iter()
            .filter_map(|definition| {
                let TypeSystemDefinition::Type(definition) = definition else {
                    return None;
                };
                let mut fields: Vec<_> = match definition.node.kind {
                    TypeKind::Object(object) => object
                        .fields
                        .into_iter()
                        .map(|field| {
                            let mut args: Vec<_> = field
                                .node
                                .arguments
                                .into_iter()
                                .map(|arg| format!("{}:{}", arg.node.name.node, arg.node.ty.node))
                                .collect();
                            args.sort();
                            format!(
                                "{}({}):{}",
                                field.node.name.node,
                                args.join(","),
                                field.node.ty.node
                            )
                        })
                        .collect(),
                    TypeKind::InputObject(input) => input
                        .fields
                        .into_iter()
                        .map(|field| format!("{}:{}", field.node.name.node, field.node.ty.node))
                        .collect(),
                    TypeKind::Enum(enumeration) => enumeration
                        .values
                        .into_iter()
                        .map(|value| value.node.value.node.to_string())
                        .collect(),
                    _ => return None,
                };
                fields.sort();
                Some((definition.node.name.node.to_string(), fields))
            })
            .collect()
    }

    #[test]
    fn offline_generated_inputs_match_served_schema_for_every_scalar_and_alias() {
        let mut fields = serde_json::Map::new();
        for (name, kind) in [
            ("price", serde_json::json!({"type":"numeric","scale":2})),
            (
                "flag",
                serde_json::json!({"type":"optional","item":{"type":"bool"}}),
            ),
            ("sequence", serde_json::json!({"type":"int64"})),
            ("owner", serde_json::json!({"type":"party"})),
            ("label", serde_json::json!({"type":"text"})),
        ] {
            fields.insert(
                name.into(),
                serde_json::json!({"op":"field","path":[name],"as":kind}),
            );
        }
        let catalog = Catalog::parse(&serde_json::json!({"version":1,"models":[{"name":"quotes","revision":1,"template":{"package_id":"0".repeat(64),"module":"Example","entity":"Quote"},"fields":fields}]}).to_string()).unwrap();
        let model = catalog.models()[0].clone();
        let binding = BoundModel {
            namespace: "demo".into(),
            fingerprint: model.fingerprint().unwrap(),
            model,
        };
        let mut alias = binding.clone();
        alias.namespace = "alias".into();
        let actual = signatures(&schema(&[binding, alias]).unwrap().sdl());
        for (name, expected) in signatures(&catalog.graphql_schema().unwrap()) {
            assert_eq!(
                actual.get(&name),
                Some(&expected),
                "schema mismatch: {name}"
            );
        }
    }
    #[test]
    fn aggregate_generated_outputs_match_served_schema_and_aliases() {
        let mut doc = serde_json::json!({"version":1,"models":[{"name":"records","revision":1,
            "template":{"package_id":"0".repeat(64),"module":"Example","entity":"Record"},
            "fields":{"state":{"op":"field","path":["state"],"as":{"type":"text"}},
                "amount":{"op":"field","path":["amount"],"as":{"type":"numeric","scale":10}},
                "sequence":{"op":"field","path":["sequence"],"as":{"type":"int64"}}}}]});
        doc["models"][0]["aggregation"] = serde_json::json!({"group_by":["state"],"measures":{
            "contracts":{"op":"count"},"total":{"op":"sum","field":"amount"},"units":{"op":"sum","field":"sequence"}}});
        doc["models"][0]["relationships"] = serde_json::json!([{"name":"related","revision":1,"source_field":"state","target":{"model":"records","revision":1,"field":"state"},"max_targets":100}]);
        doc["models"][0]["relationships"][0]["join"] = serde_json::json!({"kind":"inner","source_fields":["state"],"target_fields":["amount","sequence"]});
        let catalog = Catalog::parse(&doc.to_string()).unwrap();
        let model = catalog.models()[0].clone();
        let binding = BoundModel {
            namespace: "aggregate".into(),
            fingerprint: model.fingerprint().unwrap(),
            model,
        };
        let mut alias = binding.clone();
        alias.namespace = "alias".into();
        let actual = signatures(&schema(&[binding, alias]).unwrap().sdl());
        for (name, expected) in signatures(&catalog.graphql_schema().unwrap()) {
            assert_eq!(
                actual.get(&name),
                Some(&expected),
                "aggregate schema mismatch: {name}"
            );
        }
    }
    #[test]
    fn joined_target_types_are_distinct_across_namespace_aliases() {
        let mut target = serde_json::json!({"version":1,"models":[{"name":"children","revision":1,"template":{"package_id":"0".repeat(64),"module":"Example","entity":"Child"},"fields":{"label":{"op":"field","path":["label"],"as":{"type":"text"}},"amount":{"op":"field","path":["amount"],"as":{"type":"numeric","scale":10}},"note":{"op":"field","path":["note"],"as":{"type":"optional","item":{"type":"text"}}}}}]});
        target["models"][0]["name"] = "children".into();
        let mut source = target["models"][0].clone();
        source["name"] = "parents".into();
        source["relationships"] = serde_json::json!([{"name":"related","revision":1,"source_field":"label","target":{"model":"children","revision":1,"field":"label"},"max_targets":10,"join":{"kind":"inner","source_fields":["label"],"target_fields":["amount","note"]}}]);
        target["models"].as_array_mut().unwrap().push(source);
        let first = Catalog::parse(&target.to_string()).unwrap();
        target["models"][0]["fields"]["amount"]["as"] = serde_json::json!({"type":"text"});
        let second = Catalog::parse(&target.to_string()).unwrap();
        let mut bindings = Vec::new();
        for (namespace, catalog) in [("one", &first), ("two", &second)] {
            for model in catalog.models() {
                bindings.push(BoundModel {
                    namespace: namespace.into(),
                    model: model.clone(),
                    fingerprint: model.fingerprint().unwrap(),
                });
            }
        }
        let actual = signatures(&schema(&bindings).unwrap().sdl());
        for catalog in [&first, &second] {
            for (name, expected) in signatures(&catalog.graphql_schema().unwrap()) {
                if name == "Query" {
                    for field in expected {
                        assert!(actual[&name].contains(&field));
                    }
                } else {
                    assert_eq!(actual.get(&name), Some(&expected), "{name}");
                }
            }
        }
    }
}
