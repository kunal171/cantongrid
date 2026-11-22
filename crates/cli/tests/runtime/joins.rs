//! Typed inner-join REST/GraphQL controls with real PostgreSQL and mock source authority.
use super::*;
use cantongrid_models::Catalog;

use super::model_cleanup::Cleanup;

#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton writes"]
fn inner_join_transports_preserve_visibility_updates_cursor_domains_and_typed_values() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let namespace = format!("join_cli_{}", std::process::id());
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
    source_model["relationships"][0]["join"] = json!({"kind":"inner","source_fields":["label","note","sequence"],"target_fields":["amount","note","sequence"]});
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
        ("root", "same-key", "2.0", vec!["A"]),
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
        let _ = amount;
        payload["amount"] = "12345678901234567890.1234567891".into();
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
    let joined = format!(
        "{base}/v1/models/{namespace}/{}/joins/children/query",
        source_model.name
    );
    let relation = format!(
        "{base}/v1/models/{namespace}/{}/relationships/children/query",
        source_model.name
    );
    let other = format!(
        "{base}/v1/models/{namespace}/{}/joins/sameAmount/query",
        source_model.name
    );
    let rows = format!("{base}/v1/models/{namespace}/{}/query", model.name);
    let graph = format!("{base}/models");
    let a = jwt("a", key, clock() + 600);
    let b = jwt("b", key, clock() + 600);
    let ab = jwt("ab", key, clock() + 600);
    assert_eq!(
        request(&rt, &joined, None, Some(json!({"contract_id":"root"}))).0,
        401
    );
    assert_eq!(
        request(&rt, &other, Some(&ab), Some(json!({"contract_id":"root"}))).0,
        404
    );
    let first = request(
        &rt,
        &joined,
        Some(&ab),
        Some(json!({"contract_id":"root","limit":1})),
    )
    .1;
    assert_eq!(first["visibleTotal"], "4");
    let ac = request(
        &rt,
        &joined,
        Some(&a),
        Some(json!({"contract_id":"root","limit":1})),
    )
    .1;
    assert_eq!(ac["visibleTotal"], "3");
    assert_eq!(ac["items"][0]["targetContractId"], "a");
    let hidden = request(&rt, &joined, Some(&b), Some(json!({"contract_id":"root"})));
    let unknown = request(
        &rt,
        &joined,
        Some(&b),
        Some(json!({"contract_id":"unknown"})),
    );
    assert_eq!(hidden, unknown);
    assert_eq!(hidden.0, 404);
    let field = source_model.join_api_name("children", &model).unwrap();
    let metadata = request(&rt, &format!("{base}/v1/models"), Some(&a), None).1;
    assert!(metadata["models"].as_array().unwrap().iter().any(|m| {
        m["joinQueryFields"]
            .as_array()
            .is_some_and(|fields| fields.contains(&json!(field)))
    }));
    let selections = cantongrid_models::Relationship::OUTPUT_FIELDS.join(" ")
        + " depth joinKind sourceValues {label note sequence} targetValues {amount note sequence}";
    let operation = json!({"query":format!("query($cursor:String){{{field}(namespace:\"{namespace}\",contractId:\"root\",limit:1,cursor:$cursor){{visibleTotal nextCursor fingerprint items{{{selections}}}}}}}"),"variables":{"cursor":first["nextCursor"]}});
    let gql = request(&rt, &graph, Some(&ab), Some(operation)).1;
    assert!(gql.get("errors").is_none(), "{gql}");
    let second = request(
        &rt,
        &joined,
        Some(&ab),
        Some(json!({"contract_id":"root","limit":1,"cursor":first["nextCursor"]})),
    )
    .1;
    for name in ["items", "visibleTotal", "fingerprint"] {
        assert_eq!(gql["data"][&field][name], second[name]);
    }
    assert_eq!(
        second["items"][0]["targetValues"]["amount"],
        "12345678901234567890.1234567891"
    );
    assert_eq!(
        second["items"][0]["targetValues"]["sequence"],
        "9223372036854775807"
    );
    assert!(second["items"][0]["targetValues"]["note"].is_null());
    assert!(second["items"][0]["targetValues"].get("owner").is_none());
    assert_eq!(second["items"][0]["joinKind"], "inner");
    for (url, token, input) in [
        (
            &joined,
            &a,
            json!({"contract_id":"root","cursor":first["nextCursor"]}),
        ),
        (
            &joined,
            &ab,
            json!({"contract_id":"root2","cursor":first["nextCursor"]}),
        ),
        (
            &relation,
            &ab,
            json!({"contract_id":"root","cursor":first["nextCursor"]}),
        ),
        (&rows, &ab, json!({"cursor":first["nextCursor"]})),
        (&joined, &ab, json!({"contract_id":"root","kind":"left"})),
        (
            &joined,
            &ab,
            json!({"contract_id":"root","source_fields":["owner"]}),
        ),
        (&joined, &ab, json!({"contract_id":"root","limit":26})),
    ] {
        assert_eq!(request(&rt, url, Some(token), Some(input)).0, 400);
    }
    let relcursor = request(
        &rt,
        &relation,
        Some(&ab),
        Some(json!({"contract_id":"root","limit":1})),
    )
    .1["nextCursor"]
        .clone();
    assert_eq!(
        request(
            &rt,
            &joined,
            Some(&ab),
            Some(json!({"contract_id":"root","cursor":relcursor}))
        )
        .0,
        400
    );
    let alias = format!("{field}(namespace:\"{namespace}\",contractId:\"root\"){{visibleTotal}}");
    let denied = request(
        &rt,
        &graph,
        Some(&ab),
        Some(json!({"query":format!("{{a:{alias} b:{alias} c:{alias}}}")})),
    )
    .1;
    assert!(denied.get("errors").is_some());
    assert!(denied["data"].is_null());
    // A hidden archive cannot invalidate A's cursor, but it changes AB's visible result.
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position: "hidden-archive".into(),
                events: vec![Event {
                    id: "archive-hidden".into(),
                    contract: "00-hidden".into(),
                    template: model.template.clone(),
                    witnesses: BTreeSet::new(),
                    observed_at_micros: 2,
                    ledger_at_micros: None,
                    origin: None,
                    action: Action::ArchiveByPriorContract,
                }],
            },
            &lease(),
        )
        .unwrap();
    assert_eq!(
        request(
            &rt,
            &joined,
            Some(&a),
            Some(json!({"contract_id":"root","cursor":ac["nextCursor"]}))
        )
        .0,
        200
    );
    assert_eq!(
        request(
            &rt,
            &joined,
            Some(&ab),
            Some(json!({"contract_id":"root","cursor":first["nextCursor"]}))
        )
        .0,
        400
    );
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position: "visible-archive".into(),
                events: vec![Event {
                    id: "archive-a".into(),
                    contract: "a".into(),
                    template: model.template.clone(),
                    witnesses: BTreeSet::new(),
                    observed_at_micros: 3,
                    ledger_at_micros: None,
                    origin: None,
                    action: Action::ArchiveByPriorContract,
                }],
            },
            &lease(),
        )
        .unwrap();
    assert_eq!(
        request(
            &rt,
            &joined,
            Some(&a),
            Some(json!({"contract_id":"root","cursor":ac["nextCursor"]}))
        )
        .0,
        400
    );
    let current = request(
        &rt,
        &joined,
        Some(&ab),
        Some(json!({"contract_id":"root","limit":1})),
    )
    .1;
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(
        request(&rt, &joined, Some(&a), Some(json!({"contract_id":"root"}))).0,
        503
    );
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(
        request(&rt, &joined, Some(&a), Some(json!({"contract_id":"root"}))).0,
        503
    );
    allowed.store(true, Ordering::Relaxed);
    let mut revoked = grants.clone();
    revoked["epoch"] = 2.into();
    revoked["principals"]["ab"]["enabled"] = false.into();
    std::fs::write(root.join("grants.json"), revoked.to_string()).unwrap();
    assert_eq!(
        request(&rt, &joined, Some(&ab), Some(json!({"contract_id":"root"}))).0,
        401
    );
    let mut restored = grants;
    restored["epoch"] = 3.into();
    std::fs::write(root.join("grants.json"), restored.to_string()).unwrap();
    assert_eq!(
        request(
            &rt,
            &joined,
            Some(&ab),
            Some(json!({"contract_id":"root","cursor":current["nextCursor"]}))
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
        request(&rt, &joined, Some(&a), Some(json!({"contract_id":"root"}))).0,
        503
    );
    store
        .bind_model(&plan, &lease(), &namespace, &model.name, 1, Some(2))
        .unwrap();
    assert_eq!(
        request(&rt, &joined, Some(&a), Some(json!({"contract_id":"root"}))).0,
        200
    );
    drop(running);
    drop(cleanup);
}
