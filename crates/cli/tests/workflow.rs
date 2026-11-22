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

#[test]
fn declarative_models_validate_plan_and_preview_without_source_credentials() {
    let root = Scratch::new();
    fs::write(
        root.0.join("models.json"),
        include_str!("../assets/models/visibility.json"),
    )
    .unwrap();
    fs::write(root.0.join("payload.json"),r#"{"label":"test","owner":"A","amount":"12345678901234567890.1234567891","sequence":"9223372036854775807","note":null,"state":"Ready"}"#).unwrap();
    for action in ["validate", "plan"] {
        let result = cli(&root.0, &["models", action, "models.json"]);
        assert!(result.status.success(), "model workflow failed");
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        if action == "plan" {
            assert_eq!(value["applied"], false);
            assert!(
                value["plans"][0]["create_sql"]
                    .as_str()
                    .unwrap()
                    .contains("numeric(38,10)")
            );
        }
    }
    let result = cli(
        &root.0,
        &[
            "models",
            "preview",
            "models.json",
            "records",
            "payload.json",
        ],
    );
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        value["values"]["amount"]["value"]["decimal"],
        "12345678901234567890.1234567891"
    );
    assert_eq!(value["values"]["sequence"]["value"], "9223372036854775807");
    fs::write(
        root.0.join("invalid.json"),
        r#"{"credential":"PRIVATE_SENTINEL"}"#,
    )
    .unwrap();
    let result = cli(&root.0, &["models", "validate", "invalid.json"]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("PRIVATE_SENTINEL"));
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn model_registry_cli_persists_revision_identity_without_printing_literals() {
    const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
    // The command intentionally writes the fixture, never an arbitrary configured database.
    let mut client = cantongrid_postgres::connect_env(VARIABLE).unwrap();
    let fixture: bool = client
        .query_one(
            "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(
        fixture,
        "registry CLI test requires the disposable owned destination"
    );
    let root = Scratch::new();
    let namespace = format!(
        "cli_registry_{}_{}",
        std::process::id(),
        NEXT_REGISTRY.fetch_add(1, Ordering::Relaxed)
    );
    // Scoped cleanup also runs after an assertion failure.
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Ok(mut client) = cantongrid_postgres::connect_env(VARIABLE) {
                let _ = client.execute(
                    "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                    &[&self.0],
                );
            }
        }
    }
    static NEXT_REGISTRY: AtomicU64 = AtomicU64::new(0);
    let _cleanup = Cleanup(namespace.clone());
    assert!(
        cli(&root.0, &["storage", "init", "--database-env", VARIABLE])
            .status
            .success()
    );
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../assets/models/visibility.json")).unwrap();
    document["models"][0]["fields"]["privateNote"] = serde_json::json!({
        "op":"literal","value":{"type":"text","value":"REGISTRY_PRIVATE_SENTINEL"}
    });
    fs::write(root.0.join("models.json"), document.to_string()).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .current_dir(&root.0)
            .env_remove("CANTONGRID_SOURCE_TOKEN")
            .env_remove("CANTONGRID_API_KEY")
            .env_remove("CANTONGRID_CURSOR_KEY")
            .args(args)
            .output()
            .unwrap()
    };
    let args = [
        "models",
        "register",
        "models.json",
        "--namespace",
        &namespace,
        "--database-env",
        VARIABLE,
    ];
    for inserted in [1, 0] {
        let result = run(&args);
        assert!(result.status.success());
        assert!(!String::from_utf8_lossy(&result.stdout).contains("REGISTRY_PRIVATE_SENTINEL"));
        let body: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(body["result"]["inserted"], inserted);
        assert_eq!(body["activated"], false);
    }
    let inspect = [
        "models",
        "inspect",
        &namespace,
        "records",
        "1",
        "--database-env",
        VARIABLE,
    ];
    let before = run(&inspect);
    assert!(before.status.success());
    assert!(!String::from_utf8_lossy(&before.stdout).contains("REGISTRY_PRIVATE_SENTINEL"));
    let body: serde_json::Value = serde_json::from_slice(&before.stdout).unwrap();
    assert_eq!(body["registered"], true);
    assert_eq!(body["fingerprint"].as_str().unwrap().len(), 64);
    document["models"][0]["fields"]
        .as_object_mut()
        .unwrap()
        .remove("note");
    fs::write(root.0.join("models.json"), document.to_string()).unwrap();
    let conflict = run(&args);
    assert!(!conflict.status.success());
    assert!(conflict.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&conflict.stderr).contains("REGISTRY_PRIVATE_SENTINEL"));
    assert_eq!(run(&inspect).stdout, before.stdout);
}

#[test]
#[ignore = "requires the disposable owned PostgreSQL fixture"]
fn model_installation_cli_is_explicit_idempotent_and_redacts_schema_failures() {
    const VARIABLE: &str = "CANTONGRID_DESTINATION_URL";
    let mut client = cantongrid_postgres::connect_env(VARIABLE).unwrap();
    let fixture: bool = client
        .query_one(
            "SELECT current_database()='cantongrid' AND current_user='cantongrid_owner'",
            &[],
        )
        .unwrap()
        .get(0);
    assert!(
        fixture,
        "installation CLI control requires the disposable owned destination"
    );
    let root = Scratch::new();
    let suffix = format!(
        "{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let namespace = format!("cli_install_{suffix}");
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../assets/models/visibility.json")).unwrap();
    document["models"][0]["name"] = format!("record_{suffix}").into();
    document["models"][0]["fields"]["privateNote"] = serde_json::json!({
        "op":"literal", "value":{"type":"text","value":"INSTALLATION_PRIVATE_SENTINEL"}
    });
    let catalog = cantongrid_models::Catalog::parse(&document.to_string()).unwrap();
    let model = &catalog.models()[0];
    let plan = model.sql_plan().unwrap();
    // Test-only teardown drops only this unique model's table, after unlinking our namespace.
    struct Cleanup {
        namespace: String,
        table: String,
        fingerprint: String,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Ok(mut client) = cantongrid_postgres::connect_env(VARIABLE)
                && let Ok(mut tx) = client.transaction()
            {
                if tx
                    .execute(
                        "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
                        &[&self.namespace],
                    )
                    .is_err()
                {
                    return;
                }
                let Ok(row) = tx.query_one(
                    "SELECT count(*) FROM cantongrid.model_installations WHERE fingerprint=$1",
                    &[&self.fingerprint],
                ) else {
                    return;
                };
                if row.get::<_, i64>(0) == 0
                    && (tx
                        .batch_execute(&format!(
                            "DROP TABLE IF EXISTS cantongrid_models.\"{}\"",
                            self.table
                        ))
                        .is_err()
                        || tx
                            .execute(
                                "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1",
                                &[&self.fingerprint],
                            )
                            .is_err())
                {
                    return;
                }
                if tx
                    .execute(
                        "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                        &[&self.namespace],
                    )
                    .is_err()
                {
                    return;
                }
                let _ = tx.commit();
            }
        }
    }
    let _cleanup = Cleanup {
        namespace: namespace.clone(),
        table: plan.table.clone(),
        fingerprint: plan.fingerprint.clone(),
    };
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .current_dir(&root.0)
            .env_remove("CANTONGRID_SOURCE_TOKEN")
            .env_remove("CANTONGRID_API_KEY")
            .env_remove("CANTONGRID_CURSOR_KEY")
            .args(args)
            .output()
            .unwrap()
    };
    fs::write(root.0.join("models.json"), document.to_string()).unwrap();
    assert!(
        run(&["storage", "init", "--database-env", VARIABLE])
            .status
            .success()
    );
    assert!(
        run(&[
            "models",
            "register",
            "models.json",
            "--namespace",
            &namespace,
            "--database-env",
            VARIABLE
        ])
        .status
        .success()
    );
    let args = [
        "models",
        "install",
        &namespace,
        &model.name,
        "1",
        "--database-env",
        VARIABLE,
    ];
    for created in [true, false] {
        let result = run(&args);
        assert!(result.status.success());
        assert!(!String::from_utf8_lossy(&result.stdout).contains("INSTALLATION_PRIVATE_SENTINEL"));
        let body: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(body["result"]["created"], created);
        assert_eq!(body["result"]["linked"], created);
        assert_eq!(body["activated"], false);
    }
    client.batch_execute(&format!("ALTER TABLE cantongrid_models.\"{}\" ALTER COLUMN \"label\" SET DEFAULT 'INSTALLATION_PRIVATE_SENTINEL'", plan.table)).unwrap();
    let result = run(&args);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("INSTALLATION_PRIVATE_SENTINEL"));
    assert!(String::from_utf8_lossy(&result.stderr).contains("differs from the compiled plan"));
}

/// Offline inspection must reject invalid inputs without printing source paths or partial metadata.
#[test]
fn metadata_inspection_errors_are_redacted_and_do_not_emit_json() {
    let root = Scratch::new();
    let secret_name = "private-artifact-label.dar";
    fs::write(root.0.join(secret_name), b"private malformed contents").unwrap();
    for path in [secret_name, "missing-private-artifact.dar", "."] {
        let result = cli(&root.0, &["metadata", "inspect", path]);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(!error.contains(path));
        assert!(!error.contains("private malformed contents"));
    }
    assert!(
        !cli(&root.0, &["metadata", "inspect", secret_name, "extra"])
            .status
            .success()
    );
}

/// Offline schema sync CLI must reject invalid inputs and wrong arguments without leaking paths or contents.
#[test]
fn sync_cli_rejects_malformed_inputs_without_leaking_paths_or_payloads() {
    let root = Scratch::new();
    let old_dar = "private-old-artifact.dar";
    let new_dar = "private-new-artifact.dar";
    let models = "private-models.json";
    fs::write(root.0.join(old_dar), b"private malformed old").unwrap();
    fs::write(root.0.join(new_dar), b"private malformed new").unwrap();
    fs::write(root.0.join(models), b"private malformed models").unwrap();
    for args in [
        vec!["sync", "plan", old_dar, new_dar, models],
        vec![
            "sync",
            "evaluate",
            old_dar,
            new_dar,
            models,
            "private-fixtures.json",
        ],
        vec!["sync", "evaluate"],
        vec!["sync", "bundle", old_dar, new_dar, models, "bundle-out"],
        vec!["sync", "plan", "missing-old.dar", new_dar, models],
        vec!["sync", "plan"],
        vec!["sync", "unknown"],
    ] {
        let result = cli(&root.0, &args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(!error.contains(old_dar));
        assert!(!error.contains(new_dar));
        assert!(!error.contains(models));
        assert!(!error.contains("private malformed"));
    }
}
