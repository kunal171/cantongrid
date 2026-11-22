//! Dependency and policy regressions use synthetic LF signatures, never a live ledger.
use super::*;
use cantongrid_models::{Expression, JoinKind, JoinProjection, Relationship, RelationshipTarget};

/// Build explicit aggregate and cross-model consumers around the existing exact-scalar fixture.
fn linked() -> Catalog {
    let mut source = models().models()[0].clone();
    let mut target = source.clone();
    target.name = "targets".into();
    source.aggregation = Some(cantongrid_models::Aggregation {
        group_by: vec!["state".into()],
        measures: std::collections::BTreeMap::from([
            ("counted".into(), cantongrid_models::Measure::Count {}),
            (
                "total".into(),
                cantongrid_models::Measure::Sum {
                    field: "price".into(),
                },
            ),
        ]),
    });
    source.relationships.push(Relationship {
        name: "related".into(),
        revision: 1,
        source_field: "state".into(),
        target: RelationshipTarget {
            model: "targets".into(),
            revision: 1,
            field: "state".into(),
        },
        max_targets: 10,
        join: Some(JoinProjection {
            kind: JoinKind::Inner,
            source_fields: vec!["price".into()],
            target_fields: vec!["quantity".into()],
        }),
    });
    Catalog::parse(&serde_json::json!({"version":1,"models":[source,target]}).to_string()).unwrap()
}
#[test]
fn dependency_graph_traces_arithmetic_filters_groups_measures_keys_and_join_columns() {
    let old = metadata(&"a".repeat(64));
    let catalog = linked();
    let plan = plan::analyze(&old, &old, &catalog).unwrap();
    assert!(plan.report.compatible);
    let impact = &plan.report.impact.models[0];
    assert_eq!(impact.model, "quotes");
    assert!(!impact.affected);
    assert_eq!(impact.rebuild, "unnecessary");
    assert!(
        impact
            .fields
            .iter()
            .any(|d| d.projection == "twice" && d.path == ["price"])
    );
    for kind in [
        "filter",
        "aggregate_group",
        "aggregate_measure",
        "relationship_source",
        "join_source",
        "join_target",
    ] {
        assert!(impact.dependencies.iter().any(|d| d.kind == kind), "{kind}");
    }
    let target = impact
        .dependencies
        .iter()
        .find(|d| d.kind == "join_target")
        .unwrap();
    assert_eq!(target.fields, ["quantity", "state"]);
    assert!(!target.review);
    assert!(impact.before.aggregate_query_field.is_some());
    assert!(impact.before.join_query_fields.contains_key("related"));
}
#[test]
fn revision_refresh_reports_policy_blockers_instead_of_losing_the_entire_report() {
    let old = metadata(&"a".repeat(64));
    let new = metadata(&"b".repeat(64));
    let catalog = linked();
    let plan = plan::analyze(&old, &new, &catalog).unwrap();
    assert!(!plan.report.compatible);
    let impact = &plan.report.impact.models[0];
    assert!(impact.affected);
    assert_eq!(impact.rebuild, "blocked");
    assert_eq!(impact.proposed.revision, 2);
    assert_ne!(impact.before.sql_table, impact.proposed.sql_table);
    let target = impact
        .dependencies
        .iter()
        .find(|d| d.kind == "join_target")
        .unwrap();
    assert!(target.review);
    assert_eq!(target.target_revision, Some(1));
    assert_eq!(target.proposed_target_revision, Some(2));
    assert!(impact.proposed.join_query_fields.is_empty());
    assert!(
        plan.report
            .diagnostics
            .iter()
            .any(|d| d.field.as_deref() == Some("relationship.related"))
    );
    assert_eq!(plan.previous[0].relationships[0].target.revision, 1);
    assert!(matches!(plan.bundle(), Err(SyncError::Incompatible)));
}
#[test]
fn numeric_scale_and_optional_changes_show_exact_before_after_signatures() {
    let old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    fields(&mut new)[0].value_type = builtin("numeric", vec![s::Type::Nat { value: 11 }]);
    fields(&mut new)[2].value_type = builtin("bool", vec![]);
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    let impact = &plan.report.impact.models[0];
    let price = impact
        .fields
        .iter()
        .find(|d| d.projection == "price")
        .unwrap();
    assert!(!price.compatible);
    assert_eq!(
        price.before.as_ref().unwrap().semantics,
        serde_json::json!({"numeric_scale":10})
    );
    assert_eq!(
        price.after.as_ref().unwrap().semantics,
        serde_json::json!({"numeric_scale":11})
    );
    assert!(
        !impact
            .fields
            .iter()
            .find(|d| d.projection == "note")
            .unwrap()
            .compatible
    );
    assert!(
        impact
            .fields
            .iter()
            .find(|d| d.projection == "quantity")
            .unwrap()
            .compatible
    );
}
#[test]
fn removed_fields_are_not_guessed_as_renames_and_missing_join_targets_block_bundles() {
    let old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    fields(&mut new)[0].name = "renamedPrice".into();
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    let field = plan.report.impact.models[0]
        .fields
        .iter()
        .find(|d| d.projection == "price")
        .unwrap();
    assert_eq!(field.after_error, Some(SyncError::MissingField));
    assert!(matches!(plan.bundle(), Err(SyncError::Incompatible)));
    let mut source = linked().models()[0].clone();
    source.relationships[0].target.model = "external".into();
    let catalog =
        Catalog::parse(&serde_json::json!({"version":1,"models":[source]}).to_string()).unwrap();
    let plan = plan::analyze(&old, &old, &catalog).unwrap();
    assert!(!plan.report.compatible);
    assert!(matches!(plan.bundle(), Err(SyncError::Incompatible)));
}
#[test]
fn reports_are_reproducible_and_do_not_echo_processing_literals() {
    let old = metadata(&"a".repeat(64));
    let new = metadata(&"b".repeat(64));
    let mut model = models().models()[0].clone();
    model.filter.as_mut().unwrap().equals =
        cantongrid_core::value::Value::Text("secret-literal".into());
    model.fields.insert(
        "constant".into(),
        Expression::Literal {
            value: cantongrid_core::value::Value::Text("secret-literal".into()),
        },
    );
    let catalog =
        Catalog::parse(&serde_json::json!({"version":1,"models":[model]}).to_string()).unwrap();
    let a = plan::analyze(&old, &new, &catalog).unwrap();
    let b = plan::analyze(&old, &new, &catalog).unwrap();
    let text = serde_json::to_string(&a.report).unwrap();
    assert_eq!(text, serde_json::to_string(&b.report).unwrap());
    assert!(!text.contains("secret-literal"));
    assert_eq!(a.report.impact.models[0].rebuild, "required");
    assert_ne!(
        a.report.impact.models[0].before.query_field,
        a.report.impact.models[0].proposed.query_field
    );
}
#[test]
fn unchanged_selection_with_unselected_signature_change_needs_no_projection_rebuild() {
    let old = metadata(&"a".repeat(64));
    let mut new = old.clone();
    fields(&mut new).push(s::Field {
        name: "unused".into(),
        value_type: builtin("text", vec![]),
    });
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    assert!(!plan.report.changes.is_empty());
    assert!(!plan.report.impact.models[0].affected);
    assert_eq!(plan.report.impact.models[0].rebuild, "unnecessary");
}

#[test]
fn changing_only_a_target_also_blocks_an_unchanged_incoming_relationship() {
    let mut old = metadata(&"a".repeat(64));
    let mut target = metadata(&"c".repeat(64)).packages.remove(0);
    target.name = "target_app".into();
    old.packages.push(target);
    let mut new = metadata(&"b".repeat(64));
    new.packages[0].name = "target_app".into();
    new.packages.push(old.packages[0].clone());
    let mut models = linked().models().to_vec();
    models[1].template.package_id = "c".repeat(64);
    let catalog =
        Catalog::parse(&serde_json::json!({"version":1,"models":models}).to_string()).unwrap();
    let plan = plan::analyze(&old, &new, &catalog).unwrap();
    let impact = &plan.report.impact.models[0];
    assert_eq!(impact.before.fingerprint, impact.proposed.fingerprint);
    assert!(impact.affected);
    assert_eq!(impact.rebuild, "blocked");
    assert!(matches!(plan.bundle(), Err(SyncError::Incompatible)));
}
#[test]
fn repeated_large_signature_expansion_hits_the_report_budget() {
    let mut old = metadata(&"a".repeat(64));
    let names = (0..6000)
        .map(|n| format!("Case{n:04}{}", "x".repeat(90)))
        .collect();
    old.packages[0].data_types[1].definition = s::Definition::Enum {
        constructors: names,
    };
    let mut model = models().models()[0].clone();
    for n in 0..10 {
        model
            .fields
            .insert(format!("state{n}"), model.fields["state"].clone());
    }
    let catalog =
        Catalog::parse(&serde_json::json!({"version":1,"models":[model]}).to_string()).unwrap();
    assert!(matches!(
        plan::analyze(&old, &old, &catalog),
        Err(SyncError::Limit)
    ));
}
