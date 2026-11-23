//! Pure policy controls do not connect to sources or authenticate any supplied coverage claim.
use super::*;
use crate::ModelError;
use serde_json::{Value as Json, json};

/// Keep field/check structure explicit; every comparison uses the normal compiler and evaluator.
pub(super) fn field(name: &str, kind: Json) -> Json {
    json!({"op":"field","path":[name],"as":kind})
}
/// The default exact Numeric(10) policy compares two supplied fields, without any source authority.
pub(super) fn definition() -> Json {
    json!({"version":1,"policy":{"name":"backing","revision":1,"fields":{
        "left":field("left",json!({"type":"numeric","scale":10})),
        "right":field("right",json!({"type":"numeric","scale":10}))},
        "checks":[{"name":"balanced","left":"left","operator":"eq","right":"right"}]}})
}

/// Canonical policy identity follows semantics/revision rather than input presentation.
#[test]
fn canonical_fingerprints_ignore_formatting_map_order_and_check_order_but_bind_revision() {
    let mut p = definition();
    let mut second = p["policy"]["checks"][0].clone();
    second["name"] = "another".into();
    p["policy"]["checks"].as_array_mut().unwrap().push(second);
    let first = Policy::parse(&p.to_string()).unwrap();
    p["policy"]["checks"].as_array_mut().unwrap().reverse();
    let second = Policy::parse(&serde_json::to_string_pretty(&p).unwrap()).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.fingerprint().len(), 64);
    p["policy"]["revision"] = 2.into();
    assert_ne!(
        first.fingerprint(),
        Policy::parse(&p.to_string()).unwrap().fingerprint()
    );
    p["policy"]["checks"][0]["operator"] = "gte".into();
    assert_ne!(
        second.fingerprint(),
        Policy::parse(&p.to_string()).unwrap().fingerprint()
    );
}
/// Reject ambiguous documents and unsupported scalar relationships before fixture evaluation.
#[test]
fn malformed_policies_reject_unsupported_types_scales_references_and_ambiguous_schema() {
    let base = definition();
    for (path, value) in [
        ("/version", json!(2)),
        ("/policy/revision", json!(0)),
        ("/policy/name", json!("../bad")),
        ("/policy/fields", json!({})),
        ("/policy/checks", json!([])),
        ("/policy/checks/0/operator", json!("approx_eq")),
        ("/policy/checks/0/right", json!("unknown")),
        (
            "/policy/fields/right/as",
            json!({"type":"numeric","scale":9}),
        ),
        ("/policy/fields/right/as", json!({"type":"text"})),
        (
            "/policy/fields/right/as",
            json!({"type":"optional","item":{"type":"numeric","scale":10}}),
        ),
        (
            "/policy/fields/right/as",
            json!({"type":"numeric","scale":38}),
        ),
    ] {
        let mut invalid = base.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        assert!(Policy::parse(&invalid.to_string()).is_err(), "{path}");
    }
    let mut duplicate = base.clone();
    duplicate["policy"]["checks"]
        .as_array_mut()
        .unwrap()
        .push(base["policy"]["checks"][0].clone());
    assert!(Policy::parse(&duplicate.to_string()).is_err());
    let mut unknown = base.clone();
    unknown["policy"]["authorityVerified"] = true.into();
    assert!(Policy::parse(&unknown.to_string()).is_err());
    let duplicate_json = base
        .to_string()
        .replace("\"version\":1", "\"version\":1,\"version\":1");
    assert!(Policy::parse(&duplicate_json).is_err());
    assert!(matches!(
        Policy::parse(&" ".repeat(65537)),
        Err(ModelError::Limit)
    ));
}
/// Bound combined expression work so many small fields cannot bypass the policy budget.
#[test]
fn work_limits_apply_to_the_whole_policy_not_only_individual_expressions() {
    let mut p = definition();
    let mut expression = field("left", json!({"type":"numeric","scale":10}));
    let zero = json!({"op":"literal","value":{"type":"numeric","value":{"scale":10,"decimal":"0.0000000000"}}});
    for _ in 0..20 {
        expression = json!({"op":"add","left":expression,"right":zero});
    }
    p["policy"]["fields"]["left"] = expression;
    assert!(matches!(
        Policy::parse(&p.to_string()),
        Err(ModelError::Limit)
    ));
    let mut p = definition();
    let mut fields = serde_json::Map::new();
    for n in 0..32 {
        let mut expression = field("amount", json!({"type":"numeric","scale":10}));
        for _ in 0..4 {
            expression = json!({"op":"add","left":expression,"right":zero});
        }
        fields.insert(format!("f{n}"), expression);
    }
    p["policy"]["fields"] = fields.into();
    p["policy"]["checks"][0]["left"] = "f0".into();
    p["policy"]["checks"][0]["right"] = "f1".into();
    assert!(matches!(
        Policy::parse(&p.to_string()),
        Err(ModelError::Limit)
    ));
    let mut p = definition();
    let check = p["policy"]["checks"][0].clone();
    p["policy"]["checks"] = (0..17)
        .map(|n| {
            let mut c = check.clone();
            c["name"] = format!("c{n}").into();
            c
        })
        .collect::<Vec<_>>()
        .into();
    assert!(matches!(
        Policy::parse(&p.to_string()),
        Err(ModelError::Limit)
    ));
}
