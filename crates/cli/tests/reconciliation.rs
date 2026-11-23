//! Offline CLI controls execute outside the checkout with unusable live credentials and local fixtures.
use serde_json::{Value as Json, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

/// Own a unique temporary directory; cleanup removes only this test's supplied local inputs.
struct Scratch(PathBuf);
impl Scratch {
    /// Copy packaged fixtures so the command cannot depend on the repository working directory.
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("cantongrid-reconciliation-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join("policy.json"),
            include_str!("../assets/reconciliation/backing-policy.json"),
        )
        .unwrap();
        fs::write(
            directory.join("private-fixture.json"),
            include_str!("../assets/reconciliation/backing-fixture.json"),
        )
        .unwrap();
        Self(directory)
    }
    /// Evaluation must not require or echo environmental source tokens or SQL connection strings.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .args(args)
            .current_dir(&self.0)
            .env("CANTONGRID_SOURCE_TOKEN", "private-credential-sentinel")
            .env(
                "CANTONGRID_DESTINATION_URL",
                "invalid private-database-sentinel",
            )
            .output()
            .unwrap()
    }
}
impl Drop for Scratch {
    /// Temporary input cleanup is confined to this process's dedicated test directory.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
/// CLI success is diagnostic-only, and unsupported modes cannot mutate data or obtain authority.
#[test]
fn offline_policy_commands_remain_unverified_redacted_and_do_not_mutate_inputs() {
    let s = Scratch::new();
    let before = fs::read(s.0.join("policy.json")).unwrap();
    let validation = s.run(&["reconcile", "validate", "policy.json"]);
    assert!(validation.status.success());
    let valid: Json = serde_json::from_slice(&validation.stdout).unwrap();
    assert_eq!(valid["operation"], "reconciliation_policy_validation");
    assert_eq!(valid["sourceAccess"], "not_performed");
    let fingerprint = valid["policy"]["fingerprint"].as_str().unwrap();
    assert_eq!(fingerprint.len(), 64);
    for (fixture, expected) in [
        (
            json!({"free":"0.1","locked":"0.2","payout":"0.1","contracts":"3"}),
            "satisfied",
        ),
        (
            json!({"free":"0.1","locked":"0.1","payout":"0.1","contracts":"3","complete":true}),
            "violated",
        ),
        (json!({"free":"private-value-sentinel"}), "unavailable"),
    ] {
        let input = fixture.to_string();
        fs::write(s.0.join("private-fixture.json"), &input).unwrap();
        let evaluated = s.run(&[
            "reconcile",
            "evaluate",
            "policy.json",
            "private-fixture.json",
        ]);
        assert!(evaluated.status.success());
        let report: Json = serde_json::from_slice(&evaluated.stdout).unwrap();
        assert_eq!(report["report"]["status"], "unverified");
        assert_eq!(report["report"]["coverage"], "supplied_fixture_only");
        assert_eq!(report["report"]["completeWithinConfiguredScope"], false);
        assert!(
            report["report"]["sourceScope"].is_null() && report["report"]["sourceOffset"].is_null()
        );
        assert_eq!(report["report"]["policy"]["fingerprint"], fingerprint);
        assert_eq!(report["report"]["checks"][0]["fixtureComparison"], expected);
        assert_eq!(report["sourceAccess"], "not_performed");
        assert_eq!(report["ledgerExecution"], "not_performed");
        for output in [&evaluated.stdout, &evaluated.stderr] {
            let text = String::from_utf8_lossy(output);
            for secret in [
                "private-value-sentinel",
                "private-fixture.json",
                "private-credential-sentinel",
                "private-database-sentinel",
            ] {
                assert!(!text.contains(secret));
            }
        }
        assert_eq!(
            fs::read_to_string(s.0.join("private-fixture.json")).unwrap(),
            input
        );
    }
    for args in [
        vec![
            "reconcile",
            "evaluate",
            "policy.json",
            "private-fixture.json",
            "--write",
        ],
        vec!["reconcile", "serve", "private-fixture.json"],
        vec![
            "reconcile",
            "validate",
            "policy.json",
            "--network",
            "mainnet",
        ],
    ] {
        assert!(!s.run(&args).status.success());
    }
    assert_eq!(fs::read(s.0.join("policy.json")).unwrap(), before);
    fs::write(
        s.0.join("policy.json"),
        "private-policy-sentinel-invalid-json",
    )
    .unwrap();
    let failure = s.run(&["reconcile", "validate", "policy.json"]);
    assert!(!failure.status.success());
    assert!(!String::from_utf8_lossy(&failure.stderr).contains("private-policy-sentinel"));
    assert_eq!(fs::read_dir(&s.0).unwrap().count(), 2);
    assert!(!before.is_empty());
}
