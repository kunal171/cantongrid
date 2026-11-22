//! Explicit sample comparisons retain finite findings while discarding all projected values.
use super::*;

/// Independent before/after payloads model a contract shape change without executing a DAR.
fn samples() -> serde_json::Value {
    let payload = serde_json::json!({"price":"1.0000000000","quantity":"2","note":null,"state":"Open","contract":"secret-contract"});
    serde_json::json!({"format_version":1,"samples":[{"model":"quotes","before":payload,"after":payload}]})
}
#[test]
fn samples_compare_exact_values_and_filter_membership_without_echoing_either() {
    let old = metadata(&"a".repeat(64));
    let new = metadata(&"b".repeat(64));
    let plan = plan::analyze(&old, &new, &models()).unwrap();
    let mut input = samples();
    let report = plan.evaluate(&input.to_string()).unwrap();
    assert_eq!(report.samples[0].projected_values_equal, Some(true));
    assert_eq!(report.samples[0].before.status, "matched");
    assert_eq!(report.samples[0].after.as_ref().unwrap().revision, "2");
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("secret-contract"));
    assert!(!encoded.contains("1.0000000000"));
    input["samples"][0]["after"]["quantity"] = "3".into();
    assert_eq!(
        plan.evaluate(&input.to_string()).unwrap().samples[0].projected_values_equal,
        Some(false)
    );
    input["samples"][0]["after"]["state"] = "Closed".into();
    let report = plan.evaluate(&input.to_string()).unwrap();
    assert_eq!(report.samples[0].after.as_ref().unwrap().status, "filtered");
    assert_eq!(report.samples[0].projected_values_equal, Some(false));
}
#[test]
fn invalid_samples_and_blocked_candidates_do_not_invent_successful_evaluations() {
    let old = metadata(&"a".repeat(64));
    let mut new = metadata(&"b".repeat(64));
    fields(&mut new).remove(0);
    let blocked = plan::analyze(&old, &new, &models()).unwrap();
    let report = blocked.evaluate(&samples().to_string()).unwrap();
    assert!(!report.compatible);
    assert!(report.samples[0].after.is_none());
    assert!(report.samples[0].projected_values_equal.is_none());
    let plan = plan::analyze(&old, &old, &models()).unwrap();
    let mut input = samples();
    input["samples"][0]["after"]["quantity"] = "secret-invalid-number".into();
    let report = plan.evaluate(&input.to_string()).unwrap();
    assert_eq!(report.samples[0].after.as_ref().unwrap().status, "invalid");
    assert!(report.samples[0].projected_values_equal.is_none());
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("secret-invalid-number")
    );
}
#[test]
fn sample_documents_reject_unknowns_duplicates_nonobjects_and_resource_exhaustion() {
    let old = metadata(&"a".repeat(64));
    let plan = plan::analyze(&old, &old, &models()).unwrap();
    for change in 0..5 {
        let mut input = samples();
        match change {
            0 => input["format_version"] = 2.into(),
            1 => input["samples"][0]["model"] = "unknown-private-model".into(),
            2 => input["samples"][0]["after"] = serde_json::Value::Null,
            3 => input["samples"] = serde_json::json!([]),
            _ => input["samples"] = vec![input["samples"][0].clone(); 33].into(),
        }
        assert!(plan.evaluate(&input.to_string()).is_err());
    }
    assert!(plan.evaluate(&" ".repeat(65537)).is_err());
    assert!(
        plan.evaluate(r#"{"format_version":1,"format_version":1,"samples":[]}"#)
            .is_err()
    );
    let mut input = samples();
    input["samples"][0]["secret-unknown"] = true.into();
    assert!(plan.evaluate(&input.to_string()).is_err());
}
