//! Invalid operator shapes/budgets must fail before reading files or contacting a source.
use std::process::Command;
#[test]
fn retention_rejects_invalid_budgets_and_shapes_without_echoing_private_paths() {
    for args in [
        vec![
            "ops",
            "prune-deliveries",
            "private-runtime-file.json",
            "--retain",
            "0",
            "--limit",
            "1",
        ],
        vec![
            "ops",
            "prune-deliveries",
            "private-runtime-file.json",
            "--retain",
            "100001",
            "--limit",
            "1",
        ],
        vec![
            "ops",
            "prune-deliveries",
            "private-runtime-file.json",
            "--retain",
            "100",
            "--limit",
            "257",
        ],
        vec!["ops", "prune-deliveries", "private-runtime-file.json"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("private-runtime-file"));
    }
}

/// Create a credential-free runtime file; config validation must not contact its deliberately closed ports.
fn configuration(directory: &std::path::Path) -> serde_json::Value {
    std::fs::write(
        directory.join("grants.json"),
        r#"{"version":1,"epoch":1,"principals":{"reader":{"enabled":true,"parties":["A"]}}}"#,
    )
    .unwrap();
    serde_json::json!({"version":1,"authorization":{"endpoint":"http://127.0.0.1:9/","user":"reader","source":"config-validation","network":"localnet","generation":1,"parties":["A"],"credential_env":"CANTONGRID_CONFIG_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_CONFIG_PQS_URL","destination_database_env":"CANTONGRID_CONFIG_DESTINATION_URL","source_health":"http://127.0.0.1:9/readyz","templates":[{"package_id":"0".repeat(64),"module":"M","entity":"T"}],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":"127.0.0.1:0","issuer":"config-tests","audience":"config-api","token_key_env":"CANTONGRID_CONFIG_API_KEY","cursor_key_env":"CANTONGRID_CONFIG_CURSOR_KEY","grants_file":"grants.json"}})
}
#[test]
fn configuration_checks_pin_network_tls_and_issuer_without_credentials_or_sockets() {
    let directory =
        std::env::temp_dir().join(format!("cantongrid-config-profile-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("private-config-file.json");
    let mut document = configuration(&directory);
    let run = |action: &str, profile: &str| {
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .args(["ops", action, path.to_str().unwrap(), "--profile", profile])
            .env(
                "CANTONGRID_CONFIG_PQS_URL",
                "host=127.0.0.1 port=9 user=read_only dbname=synthetic",
            )
            .env_remove("CANTONGRID_CONFIG_SOURCE_TOKEN")
            .env_remove("CANTONGRID_CONFIG_API_KEY")
            .env_remove("CANTONGRID_CONFIG_CURSOR_KEY")
            .output()
            .unwrap()
    };
    std::fs::write(&path, document.to_string()).unwrap();
    let local = run("check-config", "localnet");
    assert!(
        local.status.success(),
        "{}",
        String::from_utf8_lossy(&local.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&local.stdout).unwrap();
    assert_eq!(output["networkRequestsPerformed"], false);
    assert_eq!(output["actualNetworkVerified"], false);
    assert!(!run("check-config", "testnet").status.success());
    document["authorization"]["network"] = "testnet".into();
    document["authorization"]["transport"] = "remote_https".into();
    document["authorization"]["endpoint"] = "https://ledger.example/".into();
    document["source_health"] = "https://pqs.example/readyz".into();
    std::fs::write(&path, document.to_string()).unwrap();
    assert!(!run("check-config", "testnet").status.success());
    document["authorization"]["expected_issuer"] = "https://issuer.example/".into();
    std::fs::write(&path, document.to_string()).unwrap();
    let remote = run("check-config", "testnet");
    assert!(
        remote.status.success(),
        "{}",
        String::from_utf8_lossy(&remote.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&remote.stdout).unwrap();
    assert_eq!(output["transport"], "remote_https");
    assert_eq!(output["actualNetworkVerified"], false);
    let live = run("preflight", "testnet");
    assert!(!live.status.success());
    assert!(live.stdout.is_empty());
    let error = String::from_utf8_lossy(&live.stderr);
    assert!(!error.contains("private-config-file"));
    assert!(!error.contains("ledger.example"));
    assert!(!run("check-config", "mainnet").status.success());
    document["source_health"] = "http://127.0.0.1:9/readyz".into();
    std::fs::write(&path, document.to_string()).unwrap();
    assert!(!run("check-config", "testnet").status.success());
    std::fs::remove_dir_all(directory).unwrap();
}
