//! Synthetic LF signatures isolate compatibility decisions; public entry points still verify DARs.
use super::*;
use cantongrid_metadata::schema as s;
use cantongrid_models::Catalog;

/// Closed builtins retain Numeric scale naturals rather than converting financial values to floats.
fn builtin(name: &str, arguments: Vec<s::Type>) -> s::Type {
    s::Type::Builtin {
        name: name.into(),
        arguments,
    }
}
/// One record signature plus unselected complex data proves that generation honors projections.
fn metadata(id: &str) -> s::Metadata {
    let root = s::TypeId {
        package_id: id.into(),
        module: "Example".into(),
        name: "Quote".into(),
    };
    let status = s::TypeId {
        name: "State".into(),
        ..root.clone()
    };
    let record = s::DataType {
        id: root.clone(),
        serializable: true,
        parameters: vec![],
        definition: s::Definition::Record {
            fields: vec![
                s::Field {
                    name: "price".into(),
                    value_type: builtin("numeric", vec![s::Type::Nat { value: 10 }]),
                },
                s::Field {
                    name: "quantity".into(),
                    value_type: builtin("int64", vec![]),
                },
                s::Field {
                    name: "note".into(),
                    value_type: builtin("optional", vec![builtin("bool", vec![])]),
                },
                s::Field {
                    name: "state".into(),
                    value_type: s::Type::Reference {
                        target: status.clone(),
                        arguments: vec![],
                    },
                },
                s::Field {
                    name: "contract".into(),
                    value_type: builtin(
                        "contract_id",
                        vec![s::Type::Reference {
                            target: root.clone(),
                            arguments: vec![],
                        }],
                    ),
                },
                s::Field {
                    name: "tags".into(),
                    value_type: builtin("list", vec![builtin("text", vec![])]),
                },
            ],
        },
    };
    s::Metadata {
        format_version: 1,
        artifact_sha256: id.into(),
        main_package_id: id.into(),
        packages: vec![s::Package {
            package_id: id.into(),
            name: "application".into(),
            version: "1.0.0".into(),
            lf_version: "2.1".into(),
            upgraded_package_id: None,
            type_dependencies: vec![],
            data_types: vec![
                record,
                s::DataType {
                    id: status,
                    serializable: true,
                    parameters: vec![],
                    definition: s::Definition::Enum {
                        constructors: vec!["Open".into(), "Closed".into()],
                    },
                },
            ],
            synonyms: vec![],
            templates: vec![s::Template {
                id: root,
                key_type: None,
                choices: vec![],
                implements: vec![],
            }],
            interfaces: vec![],
        }],
    }
}
/// The model deliberately selects exact scalars and preserves its developer-authored calculation.
fn models() -> Catalog {
    Catalog::parse(&serde_json::json!({"version":1,"models":[{"name":"quotes","revision":1,"template":{"package_id":"a".repeat(64),"module":"Example","entity":"Quote"},"fields":{
        "price":{"op":"field","path":["price"],"as":{"type":"numeric","scale":10}},
        "quantity":{"op":"field","path":["quantity"],"as":{"type":"int64"}},
        "note":{"op":"field","path":["note"],"as":{"type":"optional","item":{"type":"bool"}}},
        "state":{"op":"field","path":["state"],"as":{"type":"text"}},
        "contract":{"op":"field","path":["contract"],"as":{"type":"text"}},
        "twice":{"op":"multiply","left":{"op":"field","path":["price"],"as":{"type":"numeric","scale":10}},"right":{"op":"literal","value":{"type":"int64","value":"2"}}}
    },"filter":{"field":"state","equals":{"type":"text","value":"Open"}}}]}).to_string()).unwrap()
}
/// Return the mutable field list for targeted compatible/incompatible changes.
fn fields(metadata: &mut s::Metadata) -> &mut Vec<s::Field> {
    let s::Definition::Record { fields } = &mut metadata.packages[0].data_types[0].definition
    else {
        panic!("fixture record")
    };
    fields
}
#[test]
fn refreshed_ids_and_revision_preserve_processing_and_generate_deterministic_files() {
    let old = metadata(&"a".repeat(64));
    let new = metadata(&"b".repeat(64));
    let source = models();
    let plan = plan::analyze(&old, &new, &source).unwrap();
    assert!(plan.report.compatible);
    let result = plan.catalog.as_ref().unwrap();
    let refreshed = &result.models()[0];
    assert_eq!(refreshed.template.package_id, "b".repeat(64));
    assert_eq!(refreshed.revision, 2);
    assert!(refreshed.fields == source.models()[0].fields);
    assert!(refreshed.filter == source.models()[0].filter);
    let bundle = plan.bundle().unwrap();
    assert_eq!(
        bundle.files,
        plan::analyze(&old, &new, &source)
            .unwrap()
            .bundle()
            .unwrap()
            .files
    );
    for name in [
        "metadata.json",
        "models.json",
        "schema.sql",
        "schema.graphql",
        "types.ts",
        "compatibility.json",
        "upgrade-input.json",
        "bundle.json",
    ] {
        assert!(bundle.files.contains_key(name));
    }
    assert!(!bundle.files["models.json"].contains("tags"));
    assert!(bundle.files["schema.graphql"].contains("orderBy"));
}
#[test]
fn unchanged_packages_do_not_invent_revisions_or_activation_work() {
    let old = metadata(&"a".repeat(64));
    let source = models();
    let plan = plan::analyze(&old, &old, &source).unwrap();
    assert!(plan.report.changes.is_empty());
    assert_eq!(
        plan.catalog.as_ref().unwrap().fingerprint(),
        source.fingerprint()
    );
    assert!(plan.bundle().unwrap().files["upgrade-input.json"].contains("no_activation_required"));
}
#[test]
fn unselected_optional_additions_are_reported_without_broadening_the_projection() {
    let old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    fields(&mut new).push(s::Field {
        name: "extra".into(),
        value_type: builtin("optional", vec![builtin("text", vec![])]),
    });
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    assert!(plan.report.compatible);
    assert!(
        plan.report
            .changes
            .iter()
            .any(|c| c.kind == "added" && c.declaration.ends_with("/extra"))
    );
    assert!(!plan.bundle().unwrap().files["models.json"].contains("extra"));
}
#[test]
fn selected_numeric_optionality_removal_and_enum_changes_block_every_generated_artifact() {
    let old = metadata(&"a".repeat(64));
    for change in 0..4 {
        let mut new = metadata(&"b".repeat(64));
        match change {
            0 => {
                fields(&mut new)[0].value_type =
                    builtin("numeric", vec![s::Type::Nat { value: 11 }])
            }
            1 => fields(&mut new)[2].value_type = builtin("bool", vec![]),
            2 => {
                fields(&mut new).remove(1);
            }
            _ => {
                new.packages[0].data_types[1].definition = s::Definition::Enum {
                    constructors: vec!["Open".into(), "Closed".into(), "Invalid".into()],
                }
            }
        }
        let plan = plan::analyze(&old, &new, &models()).unwrap();
        assert!(!plan.report.compatible);
        assert!(!plan.report.diagnostics.is_empty());
        assert!(matches!(plan.bundle(), Err(SyncError::Incompatible)));
    }
}
#[test]
fn ambiguous_package_versions_are_not_selected_by_highest_version_or_matching_old_hash() {
    let old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    let mut another = new.packages[0].clone();
    another.package_id = "c".repeat(64);
    another.version = "99.0.0".into();
    new.packages.push(another);
    new.main_package_id = "d".repeat(64);
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    assert!(!plan.report.compatible);
    assert_eq!(plan.report.diagnostics[0].error, SyncError::Identity);
}
#[test]
fn dependency_defined_generic_aliases_resolve_exact_scales() {
    let mut old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    for metadata in [&mut old, &mut new] {
        let root = metadata.packages[0].data_types[0].id.clone();
        let amount = s::TypeId {
            name: "Amount".into(),
            ..root.clone()
        };
        let alias = s::TypeId {
            name: "Money".into(),
            ..root.clone()
        };
        let parameter = s::Parameter {
            name: "n".into(),
            kind: s::Kind::Nat,
        };
        metadata.packages[0].data_types.push(s::DataType {
            id: amount.clone(),
            serializable: true,
            parameters: vec![parameter.clone()],
            definition: s::Definition::Record {
                fields: vec![s::Field {
                    name: "value".into(),
                    value_type: builtin(
                        "numeric",
                        vec![s::Type::Variable {
                            name: "n".into(),
                            arguments: vec![],
                        }],
                    ),
                }],
            },
        });
        metadata.packages[0].synonyms.push(s::Synonym {
            id: alias.clone(),
            parameters: vec![parameter],
            value_type: s::Type::Reference {
                target: amount,
                arguments: vec![s::Type::Variable {
                    name: "n".into(),
                    arguments: vec![],
                }],
            },
        });
        fields(metadata).push(s::Field {
            name: "nested".into(),
            value_type: s::Type::Synonym {
                target: alias,
                arguments: vec![s::Type::Nat { value: 10 }],
            },
        });
    }
    let mut model = models().models()[0].clone();
    model.fields.insert(
        "price".into(),
        cantongrid_models::Expression::Field {
            path: vec!["nested".into(), "value".into()],
            value_type: cantongrid_models::FieldType::Numeric { scale: 10 },
        },
    );
    let catalog =
        Catalog::parse(&serde_json::json!({"version":1,"models":[model]}).to_string()).unwrap();
    assert!(
        plan::analyze(&old, &new, &catalog)
            .unwrap()
            .report
            .compatible
    );
}
#[test]
fn malformed_dars_are_rejected_at_the_public_boundary() {
    assert!(matches!(
        plan(b"not a dar", b"not a dar", "{}"),
        Err(SyncError::Metadata)
    ));
}

#[path = "impact_tests.rs"]
mod impact;

#[path = "evaluation_tests.rs"]
mod evaluation;
