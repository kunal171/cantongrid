//! Schema compatibility must not be confused with source permission or an active model binding.
use super::*;
use crate::{Catalog, Model, ModelError};
use serde_json::{Value as Json, json};

/// Canonical packaged examples remain available when this crate is tested outside the checkout.
fn inputs() -> (Policy, Model) {
    let policy = Policy::parse(include_str!(
        "../../assets/reconciliation/backing-policy.json"
    ))
    .unwrap();
    let catalog = Catalog::parse(include_str!(
        "../../assets/reconciliation/backing-model.json"
    ))
    .unwrap();
    (policy, catalog.models()[0].clone())
}
/// Inspect only the public serialized diagnostic; tests do not grant themselves source capabilities.
fn report(policy: &Policy, namespace: &str, model: &Model) -> Json {
    serde_json::to_value(policy.preflight_model(namespace, model).unwrap()).unwrap()
}
/// Projected names, not nested source paths, are the policy's schema; no coverage is inferred.
#[test]
fn projected_arithmetic_preflight_is_redacted_and_never_checks_active_bindings() {
    let (policy, model) = inputs();
    let result = report(&policy, "example-org", &model);
    assert_eq!(result["model"]["fingerprint"], model.fingerprint().unwrap());
    assert_eq!(result["policy"]["fingerprint"], policy.fingerprint());
    assert_eq!(result["model"]["revision"], "1");
    assert_eq!(result["model"]["namespace"], "example-org");
    assert_eq!(result["status"], "schema_compatible");
    assert_eq!(result["referencedFields"], 4);
    assert_eq!(result["sourceAccess"], "not_performed");
    assert_eq!(result["activeBindingChecked"], false);
    assert_eq!(result["fingerprint"].as_str().unwrap().len(), 64);
    let debug = format!(
        "{:?}",
        policy.preflight_model("example-org", &model).unwrap()
    );
    for text in [result.to_string(), debug] {
        for private in [
            "private_balances",
            "SyntheticReconciliation",
            "free",
            "payout",
        ] {
            assert!(!text.contains(private));
        }
    }
}
/// Complete definitions bind selection, revision and namespace even if output field types agree.
#[test]
fn preflight_identity_changes_with_policy_model_and_namespace_but_not_catalog_layout() {
    let (policy, model) = inputs();
    let baseline = report(&policy, "example", &model)["fingerprint"].clone();
    assert_ne!(baseline, report(&policy, "another", &model)["fingerprint"]);
    for change in [
        "revision",
        "template",
        "filter",
        "expression",
        "aggregation",
    ] {
        let mut modified = serde_json::to_value(&model).unwrap();
        match change {
            "revision" => modified["revision"] = 2.into(),
            "template" => modified["template"]["entity"] = "Another".into(),
            "filter" => {
                modified["filter"] =
                    json!({"field":"contracts","equals":{"type":"int64","value":"3"}})
            }
            "expression" => modified["fields"]["free"]["path"] = json!(["other_private_path"]),
            "aggregation" => {
                modified["aggregation"] =
                    json!({"group_by":["contracts"],"measures":{"records":{"op":"count"}}})
            }
            _ => unreachable!(),
        }
        let changed: Model = serde_json::from_value(modified).unwrap();
        assert_ne!(
            baseline,
            report(&policy, "example", &changed)["fingerprint"]
        );
    }
    let mut definition: Json = serde_json::from_str(include_str!(
        "../../assets/reconciliation/backing-policy.json"
    ))
    .unwrap();
    definition["policy"]["revision"] = 2.into();
    let changed = Policy::parse(&definition.to_string()).unwrap();
    assert_ne!(baseline, report(&changed, "example", &model)["fingerprint"]);
    let mut other = model.clone();
    other.name = "another_model".into();
    let a = json!({"version":1,"models":[model,other]});
    let mut b = a.clone();
    b["models"].as_array_mut().unwrap().reverse();
    for document in [a, b] {
        let catalog = Catalog::parse(&serde_json::to_string_pretty(&document).unwrap()).unwrap();
        let selected = catalog
            .models()
            .iter()
            .find(|m| m.name == "synthetic_backing")
            .unwrap();
        assert_eq!(
            baseline,
            report(&policy, "example", selected)["fingerprint"]
        );
    }
}
/// Reusing a familiar name cannot override missing, optional or differently scaled model outputs.
#[test]
fn missing_optional_and_inexact_model_inputs_are_rejected() {
    let (policy, model) = inputs();
    for kind in [
        json!({"type":"int64"}),
        json!({"type":"numeric","scale":9}),
        json!({"type":"optional","item":{"type":"numeric","scale":10}}),
    ] {
        let mut modified = serde_json::to_value(&model).unwrap();
        modified["fields"]["free"]["as"] = kind;
        let changed: Model = serde_json::from_value(modified).unwrap();
        assert_eq!(
            policy.preflight_model("example", &changed).unwrap_err(),
            ModelError::Type
        );
    }
    let mut changed = model.clone();
    changed.fields.remove("free");
    assert_eq!(
        policy.preflight_model("example", &changed).unwrap_err(),
        ModelError::Missing
    );
    for namespace in [
        "",
        "-invalid",
        "private.namespace",
        "private\nnamespace",
        &"x".repeat(65),
    ] {
        assert_eq!(
            policy.preflight_model(namespace, &model).unwrap_err(),
            ModelError::Definition
        );
    }
    let mut invalid = model;
    invalid.revision = 0;
    assert_eq!(
        policy.preflight_model("example", &invalid).unwrap_err(),
        ModelError::Definition
    );
}
/// Validate both arithmetic branches and unused fields; no literal can conceal an unchecked leaf.
#[test]
fn nested_raw_paths_unused_references_and_constant_only_checks_are_rejected() {
    let (_, model) = inputs();
    let base: Json = serde_json::from_str(include_str!(
        "../../assets/reconciliation/backing-policy.json"
    ))
    .unwrap();
    let mut nested = base.clone();
    nested["policy"]["fields"]["collateral"]["left"]["path"] = json!(["private_balances", "free"]);
    let policy = Policy::parse(&nested.to_string()).unwrap();
    assert_eq!(
        policy.preflight_model("example", &model).unwrap_err(),
        ModelError::Path
    );
    let mut unused = base.clone();
    unused["policy"]["fields"]["unused"] =
        json!({"op":"field","path":["missing"],"as":{"type":"int64"}});
    let policy = Policy::parse(&unused.to_string()).unwrap();
    assert_eq!(
        policy.preflight_model("example", &model).unwrap_err(),
        ModelError::Missing
    );
    // The original source expressions remain present, but this added comparison is still constant-only.
    let mut constant = base;
    constant["policy"]["fields"]["constant"] =
        json!({"op":"literal","value":{"type":"int64","value":"1"}});
    constant["policy"]["checks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"constant_only","left":"constant","operator":"eq","right":"constant"}));
    let policy = Policy::parse(&constant.to_string()).unwrap();
    assert_eq!(
        policy.preflight_model("example", &model).unwrap_err(),
        ModelError::Definition
    );
}
/// Constants are useful thresholds when a comparison still depends on one exact projected field.
#[test]
fn literal_thresholds_and_repeated_references_keep_a_single_input_identity() {
    let (_, model) = inputs();
    let policy = Policy::parse(&json!({"version":1,"policy":{"name":"nonnegative","revision":1,
        "fields":{"amount":{"op":"subtract","left":{"op":"field","path":["free"],"as":{"type":"numeric","scale":10}},"right":{"op":"field","path":["free"],"as":{"type":"numeric","scale":10}}},"zero":{"op":"literal","value":{"type":"numeric","value":{"scale":10,"decimal":"0.0000000000"}}}},
        "checks":[{"name":"same_input","left":"amount","operator":"eq","right":"zero"}]}}).to_string()).unwrap();
    assert_eq!(report(&policy, "example", &model)["referencedFields"], 1);
}
