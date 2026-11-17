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
