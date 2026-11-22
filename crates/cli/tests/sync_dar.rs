//! Optional offline CLI acceptance against the already-built PoC DAR. No source writes or SDK calls.
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
/// Command invocations run outside the checkout and cannot rely on a configured destination.
fn cli(root: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cantongrid"))
        .args(args)
        .current_dir(&root.0)
        .env(
            "CANTONGRID_DESTINATION_URL",
            "invalid-unused-offline-destination",
        )
        .output()
        .unwrap()
}
#[test]
#[ignore = "requires CANTONGRID_IMPACT_DAR pointing to the built examples/poc DAR"]
fn verified_poc_dar_reports_evaluates_and_bundles_without_a_source_connection() {
    let dar = fs::canonicalize(std::env::var("CANTONGRID_IMPACT_DAR").unwrap()).unwrap();
    let metadata = cantongrid_metadata::inspect(&fs::read(&dar).unwrap()).unwrap();
    let root =
        Scratch(std::env::temp_dir().join(format!("cantongrid-impact-{}", std::process::id())));
    fs::create_dir(&root.0).unwrap();
    fs::write(root.0.join("models.json"), serde_json::json!({"version":1,"models":[{
        "name":"markets","revision":1,"template":{"package_id":metadata.main_package_id,"module":"Market","entity":"PredictionMarket"},
        "fields":{"quantity":{"op":"field","path":["quantityLot"],"as":{"type":"int64"}}}
    }]}).to_string()).unwrap();
    fs::write(root.0.join("samples.json"), serde_json::json!({"format_version":1,"samples":[{
        "model":"markets","before":{"quantityLot":"9223372036854775807"},"after":{"quantityLot":"9223372036854775806"}
    }]}).to_string()).unwrap();
    let artifact = dar.to_str().unwrap();
    let report = cli(&root, &["sync", "plan", artifact, artifact, "models.json"]);
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(json["compatible"], true);
    assert_eq!(json["impact"]["models"][0]["rebuild"], "unnecessary");
    let evaluated = cli(
        &root,
        &[
            "sync",
            "evaluate",
            artifact,
            artifact,
            "models.json",
            "samples.json",
        ],
    );
    assert!(evaluated.status.success());
    let json: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(json["samples"][0]["projected_values_equal"], false);
    assert!(!String::from_utf8_lossy(&evaluated.stdout).contains("922337203685477580"));
    let summary = cli(
        &root,
        &["sync", "summary", artifact, artifact, "models.json"],
    );
    assert!(summary.status.success());
    let bundle = cli(
        &root,
        &[
            "sync",
            "bundle",
            artifact,
            artifact,
            "models.json",
            "bundle",
        ],
    );
    assert!(bundle.status.success());
    assert_eq!(
        summary.stdout,
        fs::read(root.0.join("bundle/impact.txt")).unwrap()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&report.stdout).unwrap(),
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(root.0.join("bundle/compatibility.json")).unwrap()
        )
        .unwrap()
    );
    // A second invocation cannot overwrite a reviewed bundle, even with identical inputs.
    assert!(
        !cli(
            &root,
            &[
                "sync",
                "bundle",
                artifact,
                artifact,
                "models.json",
                "bundle"
            ]
        )
        .status
        .success()
    );
    fs::write(root.0.join("samples.json"), b"private-invalid-samples").unwrap();
    let rejected = cli(
        &root,
        &[
            "sync",
            "evaluate",
            artifact,
            artifact,
            "models.json",
            "samples.json",
        ],
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private-invalid-samples"));
}
