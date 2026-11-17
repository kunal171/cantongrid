use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static COUNTER: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cantongrid-cli-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn cli(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cantongrid"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn init_validate_run_and_replay_work_from_another_directory() {
    let root = Scratch::new();
    assert!(cli(&root.0, &["init", "demo"]).status.success());
    assert!(
        cli(&root.0, &["validate", "demo/cantongrid.json"])
            .status
            .success()
    );
    for (args, active, history) in [
        (
            vec!["fixture", "run", "demo/cantongrid.json", "--party", "A"],
            2,
            4,
        ),
        (
            vec!["fixture", "run", "demo/cantongrid.json", "--party", "B"],
            2,
            2,
        ),
        (
            vec![
                "fixture",
                "run",
                "demo/cantongrid.json",
                "--party",
                "A",
                "--party",
                "B",
            ],
            3,
            5,
        ),
        (vec!["fixture", "run", "demo/cantongrid.json"], 0, 0),
    ] {
        let result = cli(&root.0, &args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(value["result"]["visible_active"], active);
        assert_eq!(value["result"]["visible_history"], history);
        assert_eq!(value["result"]["replay_applied_batches"], 0);
        assert!(!String::from_utf8_lossy(&result.stdout).contains("123456789"));
    }
    let before = fs::read(root.0.join("demo/cantongrid.json")).unwrap();
    assert!(!cli(&root.0, &["init", "demo"]).status.success());
    assert_eq!(
        before,
        fs::read(root.0.join("demo/cantongrid.json")).unwrap()
    );
}
#[test]
fn invalid_inputs_fail_without_credentials_paths_or_partial_success_output() {
    let root = Scratch::new();
    fs::write(root.0.join("bad.json"), r#"{"token":"PRIVATE"}"#).unwrap();
    for args in [
        vec!["validate", "bad.json"],
        vec!["validate", "PRIVATE"],
        vec!["index"],
        vec!["serve"],
    ] {
        let result = cli(&root.0, &args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE"));
    }
    assert!(cli(&root.0, &["init", "demo"]).status.success());
    assert!(
        !cli(
            &root.0,
            &["fixture", "run", "demo/cantongrid.json", "--party", "C"]
        )
        .status
        .success()
    );
    let events = root.0.join("demo/events.json");
    let text = fs::read_to_string(&events)
        .unwrap()
        .replace("\"kind\": \"archive\"", "\"kind\": \"unknown_PRIVATE\"");
    fs::write(events, text).unwrap();
    let result = cli(
        &root.0,
        &["fixture", "run", "demo/cantongrid.json", "--party", "A"],
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE"));
}

#[test]
fn input_size_and_file_kind_limits_are_enforced() {
    let root = Scratch::new();
    let large = std::fs::File::create(root.0.join("large.json")).unwrap();
    large.set_len(8 * 1024 * 1024 + 1).unwrap();
    for input in [".", "large.json"] {
        let result = cli(&root.0, &["validate", input]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
}

/// Delete only this test's synthetic source context after the CLI process exits.
struct DatabaseScope(String);
impl Drop for DatabaseScope {
    fn drop(&mut self) {
        if let Ok(mut client) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL")
            && let Ok(mut tx) = client.transaction()
        {
            for table in ["outbox", "events", "contracts", "batches", "streams"] {
                let sql = format!(
                    "DELETE FROM cantongrid.{table} WHERE stream_key IN (SELECT stream_key FROM cantongrid.streams WHERE definition->>'source'=$1)"
                );
                if tx.execute(&sql, &[&self.0]).is_err() {
                    return;
                }
            }
            let _ = tx.commit();
        }
    }
}

#[test]
#[ignore = "requires owned PostgreSQL fixture and CANTONGRID_DESTINATION_URL"]
fn durable_cli_restarts_without_duplicating_state_and_queries_scoped_counts() {
    let root = Scratch::new();
    assert!(cli(&root.0, &["init", "demo"]).status.success());
    let project = root.0.join("demo/cantongrid.json");
    let events = root.0.join("demo/events.json");
    let mut definition: serde_json::Value =
        serde_json::from_slice(&fs::read(&project).unwrap()).unwrap();
    let source = format!(
        "cli-postgres-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    definition["source"]["id"] = source.clone().into();
    fs::write(&project, definition.to_string()).unwrap();
    let mut input: serde_json::Value = serde_json::from_slice(&fs::read(&events).unwrap()).unwrap();
    input["source"]["id"] = source.clone().into();
    fs::write(&events, input.to_string()).unwrap();
    let variable = "CANTONGRID_DESTINATION_URL";
    let initialized = cli(&root.0, &["storage", "init", "--database-env", variable]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let _cleanup = DatabaseScope(source);
    for expected in [2, 0] {
        let output = cli(
            &root.0,
            &[
                "fixture",
                "index",
                "demo/cantongrid.json",
                "--database-env",
                variable,
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(data["applied_batches"], expected);
        assert_eq!(data["result"]["outbox"], 5);
    }
    // Queries recover from PostgreSQL; they do not reopen or depend on the original fixture file.
    fs::remove_file(events).unwrap();
    for (parties, active, history) in [
        (vec!["A"], 2, 4),
        (vec!["B"], 2, 2),
        (vec!["A", "B"], 3, 5),
        (vec![], 0, 0),
    ] {
        let mut args = vec![
            "fixture",
            "query",
            "demo/cantongrid.json",
            "--database-env",
            variable,
        ];
        for party in parties {
            args.extend(["--party", party]);
        }
        let output = cli(&root.0, &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(data["visible_active"], active);
        assert_eq!(data["visible_history"], history);
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(!text.contains("123456789"));
        assert!(!text.contains("local-cantongrid-only"));
    }
    assert!(
        !cli(
            &root.0,
            &[
                "fixture",
                "query",
                "demo/cantongrid.json",
                "--database-env",
                variable,
                "--party",
                "C"
            ]
        )
        .status
        .success()
    );
}

#[test]
fn source_grant_probe_rejects_bad_config_without_secret_or_partial_output() {
    let root = Scratch::new();
    fs::write(
        root.0.join("secret.json"),
        r#"{"token":"PRIVATE_SENTINEL"}"#,
    )
    .unwrap();
    fs::write(root.0.join("large.json"), vec![b' '; 65537]).unwrap();
    for path in ["secret.json", "large.json", ".", "PRIVATE_SENTINEL"] {
        let result = cli(&root.0, &["source", "verify-grants", path]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE_SENTINEL"));
    }
}

#[test]
#[ignore = "requires existing LocalNet source-grants config and current source token"]
fn existing_localnet_grants_are_verified_and_forged_bearer_is_denied() {
    let config_path = std::env::var("CANTONGRID_GRANTS_CONFIG").unwrap();
    let directory = std::env::current_dir().unwrap();
    let result = cli(&directory, &["source", "verify-grants", &config_path]);
    assert!(result.status.success(), "source grant verification failed");
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["verified"], true);
    assert_eq!(report["private_lifecycle_ingestion_enabled"], false);
    assert_eq!(report["history_coverage"], "unknown");
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    let variable = config["credential_env"].as_str().unwrap();
    let token = std::env::var(variable).unwrap();
    let mut parts: Vec<_> = token.split('.').map(str::to_owned).collect();
    assert_eq!(parts.len(), 3);
    // Preserve readable subject/expiry but replace the signature. Real Ledger auth must reject it.
    parts[2] = "A".repeat(parts[2].len());
    let result = Command::new(env!("CARGO_BIN_EXE_cantongrid"))
        .args(["source", "verify-grants", &config_path])
        .env(variable, parts.join("."))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("denied"));
}
