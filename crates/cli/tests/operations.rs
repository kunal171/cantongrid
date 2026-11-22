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
