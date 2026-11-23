//! Native SQL projections and Rust comparisons retain financial digits without approximate equality.
use super::*;

/// Large exact values and last-digit differences survive SQL; overflow dominates other complete findings.
#[test]
#[ignore = "requires owned PostgreSQL; synthetic source authority; run serially"]
fn large_values_last_digits_and_overflow_keep_exact_conservative_outcomes() {
    let s = scenario();
    let mut amount: Json =
        serde_json::from_str(&payload("9007199254740993.0000000000", "0", "Ready")).unwrap();
    amount["private_balances"]["payout"] = "1".into();
    amount["private_balances"]["contracts"] = "9007199254740993".into();
    let mut initial = batch(
        &s,
        10,
        "private-large-equal",
        &amount.to_string(),
        true,
        "A",
    );
    amount["private_balances"]["free"] = "9007199254740992.9999999999".into();
    initial
        .events
        .extend(batch(&s, 10, "private-last-digit", &amount.to_string(), true, "B").events);
    s.open()
        .apply_authorized(&s.plans[0], &initial, &authority(&s))
        .unwrap();
    let binding = bind(&s);
    let result = read(&s, &binding).unwrap();
    assert_eq!(result["comparison"], "violated");
    assert_eq!(result["checks"][0]["satisfied"], "1");
    assert_eq!(result["checks"][0]["violated"], "1");
    // Two individually valid Numeric(38,10) columns overflow when the policy adds their coefficients.
    let overflow = payload(
        "9999999999999999999999999999.9999999999",
        "0.0000000001",
        "Ready",
    );
    s.open()
        .apply_authorized(
            &s.plans[0],
            &batch(&s, 20, "private-overflow", &overflow, false, "B"),
            &authority(&s),
        )
        .unwrap();
    let result = read(&s, &binding).unwrap();
    assert_eq!(result["contracts"], "3");
    assert_eq!(result["comparison"], "unavailable");
    assert_eq!(result["checks"][0]["satisfied"], "1");
    assert_eq!(result["checks"][0]["violated"], "1");
    assert_eq!(result["checks"][0]["unavailable"], "1");
    assert_eq!(result["completeWithinConfiguredScope"], true);
    assert!(!result.to_string().contains("9007199254740993"));
}
