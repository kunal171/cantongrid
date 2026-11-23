//! Offline CLI controls execute outside the checkout with unusable live credentials and local fixtures.
use serde_json::{Value as Json, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Separate test invocations may run concurrently inside one integration-test process.
static NEXT_SCRATCH: AtomicUsize = AtomicUsize::new(0);

/// Own a unique temporary directory; cleanup removes only this test's supplied local inputs.
struct Scratch(PathBuf);
impl Scratch {
    /// Copy packaged fixtures so the command cannot depend on the repository working directory.
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "cantongrid-reconciliation-{}-{}",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join("policy.json"),
            include_str!("../assets/reconciliation/backing-policy.json"),
        )
        .unwrap();
        fs::write(
            directory.join("model.json"),
            include_str!("../assets/reconciliation/backing-model.json"),
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

/// Select models explicitly and fail closed on ambiguous/unbounded documents without accessing sources.
#[test]
fn model_preflight_checks_the_selected_schema_without_installation_or_source_access() {
    let s = Scratch::new();
    let policy = fs::read(s.0.join("policy.json")).unwrap();
    let mut catalog: Json =
        serde_json::from_str(include_str!("../assets/reconciliation/backing-model.json")).unwrap();
    let mut incompatible = catalog["models"][0].clone();
    incompatible["name"] = "aaa_incompatible".into();
    incompatible["fields"]["free"]["as"]["scale"] = 9.into();
    catalog["models"].as_array_mut().unwrap().push(incompatible);
    let document = catalog.to_string();
    fs::write(s.0.join("model.json"), &document).unwrap();
    let arguments = [
        "reconcile",
        "preflight",
        "policy.json",
        "model.json",
        "example-org",
        "synthetic_backing",
    ];
    let result = s.run(&arguments);
    assert!(result.status.success());
    let diagnostic: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(diagnostic["operation"], "reconciliation_model_preflight");
    assert_eq!(
        diagnostic["preflight"]["model"]["name"],
        "synthetic_backing"
    );
    assert_eq!(diagnostic["preflight"]["status"], "schema_compatible");
    assert_eq!(diagnostic["preflight"]["activeBindingChecked"], false);
    assert_eq!(diagnostic["sourceAccess"], "not_performed");
    assert_eq!(diagnostic["ledgerExecution"], "not_performed");
    for selected in ["aaa_incompatible", "missing_private_model"] {
        let mut args = arguments;
        args[5] = selected;
        let failed = s.run(&args);
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&failed.stderr).contains(selected));
    }
    assert_eq!(fs::read(s.0.join("policy.json")).unwrap(), policy);
    assert_eq!(
        fs::read_to_string(s.0.join("model.json")).unwrap(),
        document
    );
    for output in [&result.stdout, &result.stderr] {
        let text = String::from_utf8_lossy(output);
        for secret in [
            "private_balances",
            "SyntheticReconciliation",
            "model.json",
            "private-credential-sentinel",
            "private-database-sentinel",
        ] {
            assert!(!text.contains(secret));
        }
    }
    let mut extra = arguments.to_vec();
    extra.push("--install");
    assert!(!s.run(&extra).status.success());
    for invalid in [
        br#"{"version":1,"version":1,"models":[]}"#.to_vec(),
        b"private-catalog-sentinel".to_vec(),
        vec![b' '; 65537],
        vec![0xff],
    ] {
        fs::write(s.0.join("model.json"), &invalid).unwrap();
        let failed = s.run(&arguments);
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
        let error = String::from_utf8_lossy(&failed.stderr);
        assert!(!error.contains("private-catalog-sentinel") && !error.contains("model.json"));
        assert_eq!(fs::read(s.0.join("model.json")).unwrap(), invalid);
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
    let initial_files = fs::read_dir(&s.0).unwrap().count();
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
    assert_eq!(fs::read_dir(&s.0).unwrap().count(), initial_files);
    assert!(!before.is_empty());
}
