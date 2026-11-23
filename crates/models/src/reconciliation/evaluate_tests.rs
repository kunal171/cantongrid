//! Exact fixture comparisons remain separate from verified source evidence.
use super::definition_tests::{definition, field};
use super::*;
use serde_json::{Value as Json, json};

/// Assert finite findings through the public API, rather than reading evaluator internals.
fn finding(definition: &Json, payload: &str) -> (FixtureResult, Option<&'static str>) {
    let report = Policy::parse(&definition.to_string())
        .unwrap()
        .evaluate_fixture(payload);
    let check = &report.checks()[0];
    (check.fixture_comparison, check.error_code)
}
/// Values above 2^53 and their final decimal digit retain exact comparison meaning.
#[test]
fn exact_large_values_and_last_decimal_digit_never_round_to_equality() {
    let p = definition();
    assert_eq!(finding(&p,r#"{"left":"12345678901234567890.0000000001","right":"12345678901234567890.0000000001"}"#).0,FixtureResult::Satisfied);
    assert_eq!(finding(&p,r#"{"left":"12345678901234567890.0000000001","right":"12345678901234567890.0000000000"}"#).0,FixtureResult::Violated);
}
/// Exercise the existing checked arithmetic path through a compiled policy.
#[test]
fn checked_arithmetic_reuses_decimal_addition_and_integer_multiplication() {
    let mut p = definition();
    let n = || json!({"type":"numeric","scale":10});
    p["policy"]["fields"]["left"] =
        json!({"op":"add","left":field("free",n()),"right":field("locked",n())});
    p["policy"]["fields"]["right"] = json!({"op":"multiply","left":field("payout",n()),"right":field("contracts",json!({"type":"int64"}))});
    // Exactly 0.1 + 0.2 = 0.1 * 3, including declared trailing precision.
    assert_eq!(
        finding(
            &p,
            r#"{"free":"0.1","locked":"0.2","payout":"0.1","contracts":"3"}"#
        )
        .0,
        FixtureResult::Satisfied
    );
}
/// Check total ordering without a potentially overflowing difference calculation.
#[test]
fn ordered_comparisons_cover_negative_zero_and_extreme_coefficients_without_subtraction() {
    for (left, right, order) in [("-1", "0", -1), ("0", "0", 0), ("1", "0", 1)] {
        for (operator, expected) in [
            ("eq", order == 0),
            ("ne", order != 0),
            ("lt", order < 0),
            ("lte", order <= 0),
            ("gt", order > 0),
            ("gte", order >= 0),
        ] {
            let mut p = definition();
            p["policy"]["checks"][0]["operator"] = operator.into();
            let result = finding(&p, &json!({"left":left,"right":right}).to_string()).0;
            assert_eq!(
                result,
                if expected {
                    FixtureResult::Satisfied
                } else {
                    FixtureResult::Violated
                }
            );
        }
    }
    assert_eq!(
        finding(&definition(), r#"{"left":"-0.0000000000","right":"0"}"#).0,
        FixtureResult::Satisfied
    );
    let mut p = definition();
    for side in ["left", "right"] {
        p["policy"]["fields"][side]["as"]["scale"] = 0.into();
    }
    p["policy"]["checks"][0]["operator"] = "lt".into();
    let maximum = "9".repeat(38);
    assert_eq!(
        finding(
            &p,
            &json!({"left":format!("-{maximum}"),"right":maximum}).to_string()
        )
        .0,
        FixtureResult::Satisfied
    );
}
/// Malformed or absent required data leaves the mathematical comparisons unavailable.
#[test]
fn invalid_required_inputs_never_become_zero_balances_or_partial_success() {
    let p = definition();
    for (payload, code) in [
        (r#"{"left":null,"right":"1"}"#, "missing"),
        (r#"{"right":"1"}"#, "missing"),
        (r#"{"left":1.0,"right":"1"}"#, "type"),
        (r#"{"left":"1.00000000001","right":"1"}"#, "type"),
        (r#"{"left":"1e3","right":"1"}"#, "type"),
        (r#"{"left":"01","right":"1"}"#, "type"),
        (r#"{"left":"secret-value","left":"1","right":"1"}"#, "type"),
        ("[]", "type"),
    ] {
        assert_eq!(
            finding(&p, payload),
            (FixtureResult::Unavailable, Some(code))
        );
    }
    assert_eq!(
        finding(&p, &" ".repeat(65537)),
        (FixtureResult::Unavailable, Some("limit"))
    );
}
/// Both i64 limits and the Numeric precision bound reject overflow instead of wrapping.
#[test]
fn integer_and_decimal_arithmetic_overflow_fail_without_wrapping() {
    for kind in [json!({"type":"int64"}), json!({"type":"numeric","scale":0})] {
        let mut p = definition();
        p["policy"]["fields"]["left"] =
            json!({"op":"add","left":field("a",kind.clone()),"right":field("b",kind.clone())});
        p["policy"]["fields"]["right"] = field("zero", kind.clone());
        let maximum = if kind["type"] == "int64" {
            i64::MAX.to_string()
        } else {
            "9".repeat(38)
        };
        assert_eq!(
            finding(&p, &json!({"a":maximum,"b":"1","zero":"0"}).to_string()),
            (FixtureResult::Unavailable, Some("overflow"))
        );
        p["policy"]["fields"]["left"]["op"] = "multiply".into();
        p["policy"]["fields"]["left"]["right"] = field("b", json!({"type":"int64"}));
        let maximum = if kind["type"] == "int64" {
            i64::MAX.to_string()
        } else {
            "9".repeat(38)
        };
        assert_eq!(
            finding(&p, &json!({"a":maximum,"b":"2","zero":"0"}).to_string()),
            (FixtureResult::Unavailable, Some("overflow"))
        );
    }
}
/// Untrusted data cannot manufacture source scope or a financial success receipt.
#[test]
fn fixture_metadata_cannot_claim_authorization_completeness_or_a_live_all_clear() {
    let policy = Policy::parse(&definition().to_string()).unwrap();
    let report=policy.evaluate_fixture(r#"{"left":"1","right":"1","status":"passed","complete":true,"sourceScope":"private-party-sentinel","sourceOffset":"secret-offset-sentinel"}"#);
    assert_eq!(
        report.checks()[0].fixture_comparison,
        FixtureResult::Satisfied
    );
    let output = serde_json::to_value(&report).unwrap();
    assert_eq!(output["status"], "unverified");
    assert_eq!(output["coverage"], "supplied_fixture_only");
    assert_eq!(output["completeWithinConfiguredScope"], false);
    assert!(output["sourceScope"].is_null() && output["sourceOffset"].is_null());
    assert!(!output.to_string().contains("private-party-sentinel"));
    assert!(!output.to_string().contains("secret-offset-sentinel"));
}
/// Keep source values and expression paths out of normal and Debug diagnostics.
#[test]
fn diagnostics_and_debug_never_echo_inputs_paths_or_literals() {
    let mut p = definition();
    p["policy"]["fields"]["left"]["path"] = json!(["private_field_sentinel"]);
    let policy = Policy::parse(&p.to_string()).unwrap();
    let output = serde_json::to_string(
        &policy
            .evaluate_fixture(r#"{"private_field_sentinel":"secret-value-sentinel","right":"1"}"#),
    )
    .unwrap();
    for secret in ["private_field_sentinel", "secret-value-sentinel"] {
        assert!(!output.contains(secret));
        assert!(!format!("{policy:?}").contains(secret));
    }
}

/// Both repository examples are synthetic and remain unverified even when their exact equations hold.
#[test]
fn packaged_examples_satisfy_their_fixture_equations_without_certifying_source_state() {
    for (policy, fixture) in [
        (
            include_str!("../../assets/reconciliation/backing-policy.json"),
            include_str!("../../assets/reconciliation/backing-fixture.json"),
        ),
        (
            include_str!("../../assets/reconciliation/supply-policy.json"),
            include_str!("../../assets/reconciliation/supply-fixture.json"),
        ),
    ] {
        let compiled = Policy::parse(policy).unwrap();
        let report = compiled.evaluate_fixture(fixture);
        assert!(
            report
                .checks()
                .iter()
                .all(|c| c.fixture_comparison == FixtureResult::Satisfied)
        );
        assert_eq!(
            serde_json::to_value(report).unwrap()["status"],
            "unverified"
        );
    }
}
