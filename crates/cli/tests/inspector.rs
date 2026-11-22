//! Local evidence commands are bounded, read-only and never echo operator payloads.
use std::{fs, path::Path, process::Command};

/// Keep private test data in a dedicated temporary directory removed on both success and panic.
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn local_evaluation_emits_codes_without_payloads_paths_or_ledger_commands() {
    let directory =
        std::env::temp_dir().join(format!("cantongrid-inspector-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let scratch = Scratch(directory);
    let definitions = scratch.0.join("models.json");
    fs::write(
        &definitions,
        include_str!("../assets/models/visibility.json"),
    )
    .unwrap();
    let payload = scratch.0.join("private-input.json");
    fs::write(&payload, "{\"label\":\"secret-sentinel\"}").unwrap();
    let binary = env!("CARGO_BIN_EXE_cantongrid");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .current_dir(&scratch.0)
            .output()
            .unwrap()
    };
    let finding = run(&[
        "inspect",
        "evaluate",
        definitions.to_str().unwrap(),
        "records",
        payload.to_str().unwrap(),
    ]);
    assert!(finding.status.success());
    let report: serde_json::Value = serde_json::from_slice(&finding.stdout).unwrap();
    assert_eq!(report["diagnostic"]["status"], "invalid");
    assert_eq!(report["diagnostic"]["errorCode"], "missing");
    assert_eq!(report["ledgerExecution"], "not_performed");
    for bytes in [&finding.stdout, &finding.stderr] {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("secret-sentinel"));
        assert!(!text.contains("private-input"));
    }
    fs::write(
        &payload,
        include_str!("../assets/models/visibility-payload.json"),
    )
    .unwrap();
    let good = run(&[
        "inspect",
        "evaluate",
        definitions.to_str().unwrap(),
        "records",
        payload.to_str().unwrap(),
    ]);
    assert!(good.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&good.stdout).unwrap()["diagnostic"]["status"],
        "matched"
    );
    let invalid = run(&["inspect", "progress", "does-not-exist", "--write"]);
    assert!(!invalid.status.success());
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("cannot open"));
    let before = fs::read_dir(&scratch.0).unwrap().count();
    let unsupported = run(&[
        "inspect",
        "simulate",
        Path::new("ledger-command").to_str().unwrap(),
    ]);
    assert!(!unsupported.status.success());
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), before);
}
