//! Projection inspector REST/GraphQL controls with real PostgreSQL and mock source authority.
use super::*;
use cantongrid_models::Catalog;

use super::model_cleanup::Cleanup;

#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton writes"]
fn projection_inspector_redacts_values_and_rechecks_model_publication_and_authority() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let namespace = format!("projection_cli_{}", std::process::id());
    let root = std::env::temp_dir().join(&namespace);
    std::fs::create_dir(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let allowed = Arc::new(AtomicBool::new(true));
    let healthy = Arc::new(AtomicBool::new(true));
    let (url, source_thread) = source(stop.clone(), allowed.clone(), healthy.clone());
    let mut running = Running {
        child: None,
        directory: root.clone(),
        stop,
        source: Some(source_thread),
    };
    let mut doc: Json =
        serde_json::from_str(include_str!("../../assets/models/visibility.json")).unwrap();
    doc["models"][0]["name"] = namespace.clone().into();
    let mut source_model = doc["models"][0].clone();
    source_model["name"] = format!("{namespace}_root").into();
    source_model["filter"]["equals"]["value"] = "Anchor".into();
    source_model["relationships"] = json!([{"name":"children","revision":1,"source_field":"label","target":{"model":namespace,"revision":1,"field":"label"},"max_targets":100},{"name":"sameAmount","revision":1,"source_field":"amount","target":{"model":namespace,"revision":1,"field":"amount"},"max_targets":100}]);
    doc["models"].as_array_mut().unwrap().push(source_model);
    let catalog = Catalog::parse(&doc.to_string()).unwrap();
    let model = catalog
        .models()
        .iter()
        .find(|m| m.name == namespace)
        .unwrap()
        .clone();
    let source_model = catalog
        .models()
        .iter()
        .find(|m| m.name != namespace)
        .unwrap()
        .clone();
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
    for m in catalog.models() {
        store.install_model(&namespace, &m.name, 1).unwrap();
    }
    let events = [
        ("root", "same-key", "2.0", vec!["A", "B"]),
        ("root2", "same-key", "2.0", vec!["A", "B"]),
        ("hidden-root", "same-key", "2.0", vec!["B"]),
        ("a", "same-key", "2.0", vec!["A"]),
        ("00-hidden", "same-key", "2.0", vec!["B"]),
        ("a2", "same-key", "2.0", vec!["A"]),
        ("shared", "same-key", "2.0", vec!["A", "B"]),
    ]
    .into_iter()
    .map(|(id, label, amount, scope)| {
        let mut payload: Json =
            serde_json::from_str(include_str!("../../assets/models/visibility-payload.json"))
                .unwrap();
        payload["label"] = label.into();
        payload["amount"] = amount.into();
        if id.contains("root") {
            payload["state"] = "Anchor".into();
        }
        Event {
            id: format!("created-{id}"),
            contract: id.into(),
            template: model.template.clone(),
            witnesses: scope.into_iter().map(party).collect(),
            observed_at_micros: 1,
            ledger_at_micros: None,
            origin: None,
            action: Action::Create(Value::RawJson(payload.to_string())),
        }
    })
    .collect();
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position: "snapshot".into(),
                events,
            },
            &lease(),
        )
        .unwrap();
    store
        .bind_model(&plan, &lease(), &namespace, &model.name, 1, None)
        .unwrap();
    store
        .bind_model(&plan, &lease(), &namespace, &source_model.name, 1, None)
        .unwrap();
    let stream: String = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL")
        .unwrap()
        .query_one(
            "SELECT stream_key FROM cantongrid.model_bindings WHERE namespace=$1 LIMIT 1",
            &[&namespace],
        )
        .unwrap()
        .get(0);
    let mut cleanup = Cleanup {
        namespace: namespace.clone(),
        stream,
        models: vec![model.clone(), source_model.clone()],
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let config = json!({"version":1,"authorization":{"endpoint":url,"user":"source-reader","source":namespace,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{url}/readyz"),"templates":[model.template],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":address.to_string(),"issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
    let path = root.join("runtime.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let grants = json!({"version":1,"epoch":1,"principals":{"a":{"enabled":true,"parties":["A"]},"b":{"enabled":true,"parties":["B"]},"ab":{"enabled":true,"parties":["A","B"]}}});
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
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::net::TcpStream::connect(address).is_err() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{address}");
    let a = jwt("a", key, clock() + 600);
    let ab = jwt("ab", key, clock() + 600);
    let url = format!("{base}/v1/models/{namespace}/{}/inspect", model.name);
    let graph = format!("{base}/models");
    let field = model.diagnostic_api_name().unwrap();
    assert_eq!(
        request(&rt, &url, None, Some(json!({"contract_id":"a"}))).0,
        401
    );
    let result = request(&rt, &url, Some(&a), Some(json!({"contract_id":"a"}))).1;
    assert_eq!(result["items"][0]["status"], "matched");
    assert_eq!(result["items"][0]["projection"], "consistent");
    let text = result.to_string();
    for secret in [
        "same-key",
        "12345678901234567890",
        "9223372036854775807",
        "payload",
        "witnesses",
    ] {
        assert!(!text.contains(secret));
    }
    let selections =
        cantongrid_models::ModelDiagnostic::OUTPUT_FIELDS.join(" ") + " active errorCode";
    let operation = json!({"query":format!("{{{field}(namespace:\"{namespace}\",contractId:\"a\"){{items{{{selections}}} visibleTotal nextCursor fingerprint}}}}")});
    let gql = request(&rt, &graph, Some(&a), Some(operation)).1;
    assert!(gql.get("errors").is_none(), "{gql}");
    assert_eq!(gql["data"][&field], result);
    assert_eq!(
        request(&rt, &url, Some(&a), Some(json!({"contract_id":"root"}))).1["items"][0]["status"],
        "filtered"
    );
    let hidden = request(
        &rt,
        &url,
        Some(&a),
        Some(json!({"contract_id":"00-hidden"})),
    );
    let missing = request(&rt, &url, Some(&a), Some(json!({"contract_id":"missing"})));
    assert_eq!(hidden, missing);
    assert_eq!(hidden.0, 404);
    assert_eq!(
        request(
            &rt,
            &url,
            Some(&a),
            Some(json!({"contract_id":"a","payload":{"secret":"secret-sentinel"}}))
        )
        .0,
        400
    );
    assert_eq!(
        request(
            &rt,
            &url,
            Some(&a),
            Some(json!({"contract_id":"a","cursor":"bad"}))
        )
        .0,
        400
    );
    let mut revision = model.clone();
    revision.revision = 2;
    store
        .register_models(
            &namespace,
            &Catalog::parse(&json!({"version":1,"models":[revision]}).to_string()).unwrap(),
        )
        .unwrap();
    store.install_model(&namespace, &model.name, 2).unwrap();
    cleanup.models.push(revision);
    store
        .bind_model(&plan, &lease(), &namespace, &model.name, 2, Some(1))
        .unwrap();
    assert_eq!(
        request(&rt, &url, Some(&a), Some(json!({"contract_id":"a"}))).1["items"][0]["revision"],
        "2"
    );
    let old=request(&rt,&graph,Some(&a),Some(json!({"query":format!("{{{field}(namespace:\"{namespace}\",contractId:\"a\"){{visibleTotal}}}}") }))).1;
    assert!(old.get("errors").is_some());
    assert!(old["data"].is_null());
    store
        .bind_model(&plan, &lease(), &namespace, &model.name, 1, Some(2))
        .unwrap();
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(
        request(&rt, &url, Some(&a), Some(json!({"contract_id":"a"}))).0,
        503
    );
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    let before_progress = store.checkpoint(&plan).unwrap();
    let denied = Command::new(env!("CARGO_BIN_EXE_cantongrid"))
        .args(["inspect", "progress", path.to_str().unwrap()])
        .env("CANTONGRID_TEST_PQS_URL", source_database)
        .env(
            "CANTONGRID_TEST_SOURCE_TOKEN",
            jwt("source-reader", key, clock() + 600),
        )
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert!(denied.stdout.is_empty());
    assert_eq!(store.checkpoint(&plan).unwrap(), before_progress);
    let error = String::from_utf8_lossy(&denied.stderr);
    assert!(!error.contains(source_database));
    assert!(!error.contains("secret-sentinel"));

    assert_eq!(
        request(&rt, &url, Some(&a), Some(json!({"contract_id":"a"}))).0,
        503
    );
    allowed.store(true, Ordering::Relaxed);
    let mut revoked = grants;
    revoked["epoch"] = 2.into();
    revoked["principals"]["ab"]["enabled"] = false.into();
    std::fs::write(root.join("grants.json"), revoked.to_string()).unwrap();
    assert_eq!(
        request(&rt, &url, Some(&ab), Some(json!({"contract_id":"a"}))).0,
        401
    );
    drop(running);
    drop(cleanup);
}
