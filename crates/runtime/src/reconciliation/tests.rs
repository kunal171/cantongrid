//! Offline manifest controls use explicitly owned files and never Project, a database or source grants.
use super::*;
use serde_json::{Value as Json, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
/// Own each test's local manifest/policy files independently, including under parallel execution.
struct Files(PathBuf);
impl Files {
    /// Packaged inputs keep these controls runnable from an unpacked runtime crate.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cantongrid-policy-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("private-policy.json"),
            include_str!("../../assets/reconciliation/backing-policy.json"),
        )
        .unwrap();
        Self(path)
    }
    /// Use a syntactically valid pin; actual installation/activation belongs to the authorized API reader.
    fn definition(&self) -> Json {
        let policy = Policy::parse(include_str!(
            "../../assets/reconciliation/backing-policy.json"
        ))
        .unwrap();
        json!({"version":1,"policies":[{"name":"backing","namespace":"example-org","model":"synthetic_backing","model_revision":1,"model_fingerprint":"a".repeat(64),"policy_file":"private-policy.json","policy_fingerprint":policy.fingerprint()}]})
    }
    /// Write only this test's explicit manifest; source paths never appear in returned diagnostics.
    fn load(&self, definition: &Json) -> Result<Catalog, String> {
        let path = self.0.join("manifest.json");
        std::fs::write(&path, definition.to_string()).unwrap();
        Catalog::load(Some(&path))
    }
}
impl Drop for Files {
    /// Cleanup is limited to the uniquely owned directory, regardless of assertion failures.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// Semantic identity binds aliases and immutable pins, while allowing reusable policies and file reformatting.
#[test]
fn catalog_identity_is_canonical_and_does_not_grant_active_binding_authority() {
    let files = Files::new();
    let mut definition = files.definition();
    let mut alias = definition["policies"][0].clone();
    alias["name"] = "another".into();
    alias["namespace"] = "other-org".into();
    definition["policies"].as_array_mut().unwrap().push(alias);
    let a = files.load(&definition).unwrap();
    assert_eq!(a.entries.len(), 2);
    assert_eq!(a.entries[0].pin.name, "another");
    assert_eq!(a.entries[0].policy.name(), "collateral_backing");
    definition["policies"].as_array_mut().unwrap().reverse();
    assert_eq!(a.fingerprint, files.load(&definition).unwrap().fingerprint);
    definition["policies"][0]["model_revision"] = 2.into();
    assert_ne!(a.fingerprint, files.load(&definition).unwrap().fingerprint);
    let metadata = serde_json::to_string(&a.entries[0].pin).unwrap();
    assert!(!metadata.contains("private-policy") && !metadata.contains("fields"));
    assert_eq!(a.entries[0].pin.model_revision, "1");
    assert!(Catalog::load(None).unwrap().entries.is_empty());
}
/// Pin/version/name/schema failures must be finite errors rather than accepting unsupported execution inputs.
#[test]
fn invalid_versions_shapes_and_pins_fail_without_echoing_definitions_or_paths() {
    let files = Files::new();
    let base = files.definition();
    for (path, value) in [
        ("/version", json!(2)),
        ("/policies", json!([])),
        ("/policies/0/name", json!("bad.name")),
        ("/policies/0/namespace", json!("-bad")),
        ("/policies/0/model_revision", json!(0)),
        ("/policies/0/model_fingerprint", json!("A".repeat(64))),
        ("/policies/0/policy_fingerprint", json!("b".repeat(64))),
        (
            "/policies/0/policy_file",
            json!("private-missing-sentinel.json"),
        ),
    ] {
        let mut changed = base.clone();
        *changed.pointer_mut(path).unwrap() = value;
        let error = files.load(&changed).err().unwrap();
        assert!(!error.contains("private-") && !error.contains("collateral_backing"));
    }
    let mut duplicate = base.clone();
    duplicate["policies"]
        .as_array_mut()
        .unwrap()
        .push(base["policies"][0].clone());
    assert!(files.load(&duplicate).is_err());
    let mut unknown = base.clone();
    unknown["policies"][0]["sql"] = "private-sql-sentinel".into();
    assert!(files.load(&unknown).is_err());
    let mut excess = base.clone();
    excess["policies"] = (0..17)
        .map(|n| {
            let mut p = base["policies"][0].clone();
            p["name"] = format!("policy_{n}").into();
            p
        })
        .collect::<Vec<_>>()
        .into();
    assert!(files.load(&excess).is_err());
    for bytes in [
        br#"{"version":1,"version":1,"policies":[]}"#.to_vec(),
        vec![b' '; 65537],
        vec![0xff],
    ] {
        let path = files.0.join("manifest.json");
        std::fs::write(&path, bytes).unwrap();
        assert!(Catalog::load(Some(&path)).is_err());
    }
}
