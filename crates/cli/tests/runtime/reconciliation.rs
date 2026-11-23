//! Owned PostgreSQL and mock source authority validate both transports; no Canton transactions occur.
use super::model_cleanup::Cleanup;
use super::*;
use cantongrid_models::{Catalog, reconciliation::Policy};
use std::sync::{Mutex, atomic::AtomicUsize};

/// Only the synthetic source thread can execute these owned-fixture actions.
type HookAction = Box<dyn FnOnce() + Send>;
/// Arm one callback on the final source refresh after an evaluation has completed.
struct Barrier {
    count: Arc<AtomicUsize>,
    action: Arc<Mutex<Option<(usize, HookAction)>>>,
}
impl Barrier {
    /// The three refreshes are admission, evaluation and final response delivery.
    fn final_refresh(&self, action: impl FnOnce() + Send + 'static) {
        *self.action.lock().unwrap() =
            Some((self.count.load(Ordering::SeqCst) + 3, Box::new(action)));
    }
    /// Prove a denial occurred after the intended boundary rather than at initial admission.
    fn fired(&self) {
        assert!(self.action.lock().unwrap().is_none());
    }
}

/// A completed schema041 handoff is required even for a single active input contract.
fn position() -> String {
    json!({"snapshot":false,"boundary":10,"through":10,"offset":10,"pk":i64::MAX}).to_string()
}

/// Exercise real HTTP, active model SQL and final authorization with only synthetic local authority.
#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton writes"]
fn reconciliation_transports_pin_policies_and_fail_closed_on_scope_or_state_changes() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let namespace = format!("reconciliation_cli_{}", std::process::id());
    let root = std::env::temp_dir().join(&namespace);
    std::fs::create_dir(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let allowed = Arc::new(AtomicBool::new(true));
    let healthy = Arc::new(AtomicBool::new(true));
    let barrier = Barrier {
        count: Arc::new(AtomicUsize::new(0)),
        action: Arc::new(Mutex::new(None)),
    };
    let count = barrier.count.clone();
    let action = barrier.action.clone();
    let (url, source_thread) = source_with_hook(
        stop.clone(),
        allowed.clone(),
        healthy.clone(),
        move |head| {
            if head.starts_with("GET /v2/users/source-reader ") {
                let ordinal = count.fetch_add(1, Ordering::SeqCst) + 1;
                let run = {
                    let mut pending = action.lock().unwrap();
                    if pending.as_ref().is_some_and(|(at, _)| *at == ordinal) {
                        pending.take()
                    } else {
                        None
                    }
                };
                if let Some((_, run)) = run {
                    run();
                }
            }
        },
    );
    let mut running = Running {
        child: None,
        directory: root.clone(),
        stop,
        source: Some(source_thread),
    };
    let mut document: Json = serde_json::from_str(include_str!(
        "../../assets/reconciliation/backing-model.json"
    ))
    .unwrap();
    document["models"][0]["name"] = namespace.clone().into();
    let catalog = Catalog::parse(&document.to_string()).unwrap();
    let model = catalog.models()[0].clone();
    let source_database = "host=127.0.0.1 port=9 user=read_only dbname=synthetic";
    let context = SourceContext::new(namespace.clone(), "synthetic".into(), 1).unwrap();
    let plan = Plan::pqs(
        context.clone(),
        [party("A"), party("B")].into(),
        [model.template.clone()].into(),
    )
    .unwrap()
    .with_source_binding(cantongrid_runtime::config::source_binding(
        &url,
        "source-reader",
        &cantongrid_postgres::identity_local(source_database).unwrap(),
        &format!("{url}/readyz"),
    ));
    let lease = || {
        SourceLease::from_verified(
            context.binding(),
            [party("A"), party("B")],
            Instant::now(),
            MAX_SOURCE_LEASE,
            Instant::now() + Duration::from_secs(60),
        )
        .unwrap()
    };
    let mut store = Store::open_env("CANTONGRID_DESTINATION_URL").unwrap();
    store.register_models(&namespace, &catalog).unwrap();
    store.install_model(&namespace, &model.name, 1).unwrap();
    let values: Json = serde_json::from_str(include_str!(
        "../../assets/reconciliation/backing-fixture.json"
    ))
    .unwrap();
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position: position(),
                events: vec![Event {
                    id: "private-created-marker".into(),
                    contract: "private-contract-marker".into(),
                    template: model.template.clone(),
                    witnesses: [party("A")].into(),
                    observed_at_micros: 1,
                    ledger_at_micros: None,
                    origin: Some(cantongrid_core::lifecycle::Origin {
                        offset: 10,
                        snapshot_observation: true,
                    }),
                    action: Action::Create(Value::RawJson(
                        json!({"private_balances":values}).to_string(),
                    )),
                }],
            },
            &lease(),
        )
        .unwrap();
    store
        .bind_model(&plan, &lease(), &namespace, &model.name, 1, None)
        .unwrap();
    let stream: String = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL")
        .unwrap()
        .query_one(
            "SELECT stream_key FROM cantongrid.model_bindings WHERE namespace=$1",
            &[&namespace],
        )
        .unwrap()
        .get(0);
    let mut cleanup = Cleanup {
        namespace: namespace.clone(),
        stream: stream.clone(),
        models: vec![model.clone()],
    };
    let policy_text = include_str!("../../assets/reconciliation/backing-policy.json");
    let policy = Policy::parse(policy_text).unwrap();
    std::fs::write(root.join("private-policy.json"), policy_text).unwrap();
    let manifest = json!({"version":1,"policies":[{"name":"backing","namespace":namespace,"model":model.name,"model_revision":1,"model_fingerprint":model.fingerprint().unwrap(),"policy_file":"private-policy.json","policy_fingerprint":policy.fingerprint()}]});
    std::fs::write(root.join("policies.json"), manifest.to_string()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let config = json!({"version":1,"authorization":{"endpoint":url,"user":"source-reader","source":namespace,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{url}/readyz"),"reconciliation_file":"policies.json","templates":[model.template],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":address.to_string(),"issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
    let path = root.join("runtime.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let grants = json!({"version":1,"epoch":1,"principals":{"a":{"enabled":true,"parties":["A"]},"ab":{"enabled":true,"parties":["A","B"]}}});
    std::fs::write(root.join("grants.json"), grants.to_string()).unwrap();
    let key = "local-owned-fixture-organization-key-0001";
    running.child = Some(
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .args(["serve", path.to_str().unwrap()])
            .env("CANTONGRID_TEST_PQS_URL", source_database)
            .env(
                "CANTONGRID_TEST_SOURCE_TOKEN",
                jwt("source-reader", key, clock() + 600),
            )
            .env("CANTONGRID_TEST_API_KEY", key)
            .env(
                "CANTONGRID_TEST_CURSOR_KEY",
                "local-cursor-test-key-separated-00000001",
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    for _ in 0..100 {
        if std::net::TcpStream::connect(address).is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = format!("http://{address}");
    let rest = format!("{base}/v1/reconciliation/backing/query");
    let missing = format!("{base}/v1/reconciliation/private_missing/query");
    let graph = format!("{base}/reconciliation");
    let ab = jwt("ab", key, clock() + 600);
    let a = jwt("a", key, clock() + 600);
    assert_eq!(request(&rt, &rest, None, Some(json!({}))).0, 401);
    for endpoint in [&rest, &missing] {
        assert_eq!(request(&rt, endpoint, Some(&a), Some(json!({}))).0, 403);
    }
    assert_eq!(
        request(
            &rt,
            &graph,
            Some(&a),
            Some(json!({"query":"{ reconciliation(name: \"private_missing\") { contracts } }"}))
        )
        .0,
        403
    );
    assert_eq!(request(&rt, &missing, Some(&ab), Some(json!({}))).0, 404);
    let (status, metadata) = request(&rt, &format!("{base}/v1/reconciliation"), Some(&ab), None);
    assert_eq!(status, 200);
    assert_eq!(metadata["policies"][0]["name"], "backing");
    assert_eq!(
        metadata["policies"][0]["policyFingerprint"],
        policy.fingerprint()
    );
    let (status, result) = request(&rt, &rest, Some(&ab), Some(json!({})));
    assert_eq!(status, 200);
    assert_eq!(result["comparison"], "satisfied");
    assert_eq!(result["contracts"], "1");
    // 0.1 + 0.2 equals 0.1 * 3 exactly, with identical typed results over both transports.
    let fields = "
        formatVersion
        policy { name revision fingerprint }
        model { namespace name revision fingerprint }
        definitionFingerprint
        status
        comparison
        coverage
        completeWithinConfiguredScope
        contracts
        checks { name satisfied violated unavailable }
    ";
    let query = format!("{{ reconciliation(name: \"backing\") {{ {fields} }} }}");
    let (status, gql) = request(&rt, &graph, Some(&ab), Some(json!({"query":query})));
    assert_eq!(status, 200);
    assert!(gql.get("errors").is_none(), "{gql}");
    assert_eq!(gql["data"]["reconciliation"], result);
    for text in [result.to_string(), metadata.to_string(), gql.to_string()] {
        for marker in [
            "private-policy",
            "private_balances",
            "private-contract-marker",
            "0.1000000000",
            "sourceOffset",
            "witnesses",
        ] {
            assert!(!text.contains(marker));
        }
    }
    for unsupported in [
        json!({"parties":["A","B"]}),
        json!({"policy":{}}),
        json!({"limit":1}),
        json!({"path":"private-policy.json"}),
        json!([]),
    ] {
        assert_eq!(request(&rt, &rest, Some(&ab), Some(unsupported)).0, 400);
    }
    let mixed=request(&rt,&graph,Some(&ab),Some(json!({"query":"{ good: reconciliation(name: \"backing\") { contracts } missing: reconciliation(name: \"private_missing\") { contracts } }"}))).1;
    assert!(mixed["data"].is_null() && mixed["errors"].is_array());
    let excess=request(&rt,&graph,Some(&ab),Some(json!({"query":"{ a: reconciliation(name: \"backing\") { contracts } b: reconciliation(name: \"backing\") { contracts } c: reconciliation(name: \"backing\") { contracts } }"}))).1;
    assert!(excess["data"].is_null() && excess["errors"].is_array());
    std::fs::write(
        root.join("private-policy.json"),
        "private-invalid-policy-marker",
    )
    .unwrap();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    // Unrelated canonical reads do not load optional policy contents.
    assert_eq!(
        request(&rt, &format!("{base}/v1/contracts"), Some(&ab), None).0,
        200
    );
    std::fs::write(root.join("private-policy.json"), policy_text).unwrap();
    let mut bad = manifest.clone();
    bad["policies"][0]["policy_fingerprint"] = "b".repeat(64).into();
    std::fs::write(root.join("policies.json"), bad.to_string()).unwrap();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    std::fs::write(root.join("policies.json"), manifest.to_string()).unwrap();
    // Change a valid policy only after arithmetic work; the old report must still be discarded.
    let mut changed_policy: Json = serde_json::from_str(policy_text).unwrap();
    changed_policy["policy"]["revision"] = 2.into();
    let changed = Policy::parse(&changed_policy.to_string()).unwrap();
    let mut changed_manifest = manifest.clone();
    changed_manifest["policies"][0]["policy_fingerprint"] = changed.fingerprint().into();
    let directory = root.clone();
    barrier.final_refresh(move || {
        std::fs::write(
            directory.join("private-policy.json"),
            changed_policy.to_string(),
        )
        .unwrap();
        std::fs::write(
            directory.join("policies.json"),
            changed_manifest.to_string(),
        )
        .unwrap();
    });
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    barrier.fired();
    std::fs::write(root.join("private-policy.json"), policy_text).unwrap();
    std::fs::write(root.join("policies.json"), manifest.to_string()).unwrap();
    let late_allowed = allowed.clone();
    barrier.final_refresh(move || late_allowed.store(false, Ordering::Relaxed));
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    barrier.fired();
    allowed.store(true, Ordering::Relaxed);
    let directory = root.clone();
    let mut late_grants = grants.clone();
    late_grants["epoch"] = 2.into();
    late_grants["principals"]["ab"]["enabled"] = false.into();
    barrier.final_refresh(move || {
        std::fs::write(directory.join("grants.json"), late_grants.to_string()).unwrap()
    });
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 401);
    barrier.fired();
    std::fs::write(root.join("grants.json"), grants.to_string()).unwrap();
    // Overflow is a complete diagnostic with HTTP 200, not a successful mathematical finding.
    let overflow = json!({"private_balances":{"free":"9999999999999999999999999999.9999999999","locked":"0.0000000001","payout":"0.1","contracts":"3"}});
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position:
                    json!({"snapshot":false,"boundary":20,"through":20,"offset":20,"pk":i64::MAX})
                        .to_string(),
                events: vec![Event {
                    id: "private-overflow-create".into(),
                    contract: "private-overflow-contract".into(),
                    template: model.template.clone(),
                    witnesses: [party("B")].into(),
                    observed_at_micros: 2,
                    ledger_at_micros: None,
                    origin: Some(cantongrid_core::lifecycle::Origin {
                        offset: 20,
                        snapshot_observation: false,
                    }),
                    action: Action::Create(Value::RawJson(overflow.to_string())),
                }],
            },
            &lease(),
        )
        .unwrap();
    let (status, diagnostic) = request(&rt, &rest, Some(&ab), Some(json!({})));
    assert_eq!(status, 200);
    assert_eq!(diagnostic["comparison"], "unavailable");
    assert_eq!(diagnostic["checks"][0]["unavailable"], "1");
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    allowed.store(true, Ordering::Relaxed);
    let mut revoked = grants.clone();
    revoked["epoch"] = 2.into();
    revoked["principals"]["ab"]["enabled"] = false.into();
    std::fs::write(root.join("grants.json"), revoked.to_string()).unwrap();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 401);
    let mut restored = grants;
    restored["epoch"] = 3.into();
    std::fs::write(root.join("grants.json"), restored.to_string()).unwrap();
    let mut revision = model.clone();
    revision.revision = 2;
    store
        .register_models(
            &namespace,
            &Catalog::parse(&json!({"version":1,"models":[revision]}).to_string()).unwrap(),
        )
        .unwrap();
    store.install_model(&namespace, &model.name, 2).unwrap();
    cleanup.models.push(revision.clone());
    let late_plan = plan.clone();
    let late_namespace = namespace.clone();
    let late_name = model.name.clone();
    barrier.final_refresh(move || {
        let now = Instant::now();
        let authority = SourceLease::from_verified(
            late_plan.context().binding(),
            [party("A"), party("B")],
            now,
            MAX_SOURCE_LEASE,
            now + Duration::from_secs(60),
        )
        .unwrap();
        Store::open_env("CANTONGRID_DESTINATION_URL")
            .unwrap()
            .bind_model(
                &late_plan,
                &authority,
                &late_namespace,
                &late_name,
                2,
                Some(1),
            )
            .unwrap();
    });
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    barrier.fired();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    let mut promoted = manifest;
    promoted["policies"][0]["model_revision"] = 2.into();
    promoted["policies"][0]["model_fingerprint"] = revision.fingerprint().unwrap().into();
    std::fs::write(root.join("policies.json"), promoted.to_string()).unwrap();
    store.observe_authorized(&plan, &lease()).unwrap();
    assert_eq!(
        request(&rt, &rest, Some(&ab), Some(json!({}))).1["model"]["revision"],
        "2"
    );
    let mut client = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    client.execute("UPDATE cantongrid.streams SET observed_at=now()-interval '31 seconds' WHERE stream_key=$1",&[&stream]).unwrap();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    store.observe_authorized(&plan, &lease()).unwrap();
    client
        .execute(
            "DELETE FROM cantongrid.batches WHERE stream_key=$1",
            &[&stream],
        )
        .unwrap();
    assert_eq!(request(&rt, &rest, Some(&ab), Some(json!({}))).0, 503);
    drop(running);
    drop(cleanup);
}
