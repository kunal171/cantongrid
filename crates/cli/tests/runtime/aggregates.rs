//! Aggregate transport/privacy controls against PostgreSQL and a mock read-only source authority.
use super::*;
use cantongrid_models::Catalog;

/// Release only this test's generated tables, registry and source stream, even after a panic.
struct Cleanup {
    namespace: String,
    stream: String,
    models: Vec<cantongrid_models::Model>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        let Ok(mut client) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL") else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            for model in &self.models {
                tx.batch_execute(&format!(
                    "DROP TABLE IF EXISTS cantongrid_models.\"{}\"",
                    model.sql_plan()?.table
                ))?;
            }
            for table in [
                "model_rebuild_dirty",
                "model_rebuilds",
                "model_bindings",
                "outbox",
                "events",
                "contracts",
                "batches",
                "streams",
            ] {
                tx.execute(
                    &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                    &[&self.stream],
                )?;
            }
            tx.execute(
                "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
                &[&self.namespace],
            )?;
            tx.execute(
                "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
                &[&self.namespace],
            )?;
            for model in &self.models {
                tx.execute(
                    "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1",
                    &[&model.fingerprint()?],
                )?;
            }
            Ok(())
        })();
        if result.is_ok() {
            let _ = tx.commit();
        }
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton writes"]
fn aggregate_rest_graphql_share_privacy_cursors_and_revocation_guards() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let namespace = format!("aggregate_cli_{}", std::process::id());
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
    doc["models"][0]["aggregation"] = json!({"group_by":["label"],"measures":{
        "contracts":{"op":"count"},"total":{"op":"sum","field":"amount"},"units":{"op":"sum","field":"sequence"}}});
    let catalog = Catalog::parse(&doc.to_string()).unwrap();
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
    let events = [
        ("a", "alone", "2.0", vec!["A"]),
        ("b", "mixed", "3.0", vec!["B"]),
        ("a2", "mixed", "4.0", vec!["A"]),
        ("shared", "shared", "0.1", vec!["A", "B"]),
    ]
    .into_iter()
    .map(|(id, label, amount, scope)| {
        let mut payload: Json =
            serde_json::from_str(include_str!("../../assets/models/visibility-payload.json"))
                .unwrap();
        payload["label"] = label.into();
        payload["amount"] = amount.into();
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
        stream,
        models: vec![model.clone()],
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
    let aggregate = format!("{base}/v1/models/{namespace}/{}/aggregate", model.name);
    let rows = format!("{base}/v1/models/{namespace}/{}/query", model.name);
    let a = jwt("a", key, clock() + 600);
    let b = jwt("b", key, clock() + 600);
    let ab = jwt("ab", key, clock() + 600);
    assert_eq!(request(&rt, &aggregate, None, Some(json!({}))).0, 401);
    // A denial is independent of group membership; no hidden-input presence oracle.
    assert_eq!(request(&rt, &aggregate, Some(&a), Some(json!({}))).0, 403);
    assert_eq!(request(&rt, &aggregate, Some(&b), Some(json!({}))).0, 403);
    let listing = request(&rt, &format!("{base}/v1/models"), Some(&ab), None).1;
    let (_, described) = request(
        &rt,
        &format!("{base}/models"),
        Some(&ab),
        Some(
            json!({"query":"{models{namespace name revision fingerprint queryField fields{name kind nullable scale operators} aggregateQueryField aggregateFields{name kind nullable scale operators}}}"}),
        ),
    );
    assert_eq!(described["data"]["models"], listing["models"]);
    assert_eq!(
        listing["models"][0]["aggregateQueryField"],
        model.aggregate_api_name().unwrap()
    );
    assert!(
        listing["models"][0]["aggregateFields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["operators"] == json!([]))
    );
    let first = request(&rt, &aggregate, Some(&ab), Some(json!({"limit":1}))).1;
    let field = model.aggregate_api_name().unwrap();
    let graph = format!("{base}/models");
    let operation = json!({"query":format!("query($cursor:String){{{field}(namespace:\"{namespace}\",limit:1,cursor:$cursor){{visibleTotal nextCursor fingerprint items{{groupKey values{{label contracts total units}}}}}}}}"),"variables":{"cursor":first["nextCursor"]}});
    let (_, gql) = request(&rt, &graph, Some(&ab), Some(operation));
    assert!(gql.get("errors").is_none(), "{gql}");
    let second = request(
        &rt,
        &aggregate,
        Some(&ab),
        Some(json!({"limit":1,"cursor":first["nextCursor"]})),
    )
    .1;
    assert_eq!(gql["data"][&field]["items"], second["items"]);
    assert_eq!(second["items"][0]["values"]["contracts"], "2");
    assert_eq!(second["items"][0]["values"]["total"], "7.0000000000");
    assert_eq!(
        second["items"][0]["values"]["units"],
        "18446744073709551614"
    );
    assert_eq!(
        request(
            &rt,
            &aggregate,
            Some(&a),
            Some(json!({"cursor":first["nextCursor"]}))
        )
        .0,
        400
    );
    assert_eq!(
        request(
            &rt,
            &rows,
            Some(&ab),
            Some(json!({"cursor":first["nextCursor"]}))
        )
        .0,
        400
    );
    let row_cursor =
        request(&rt, &rows, Some(&ab), Some(json!({"limit":1}))).1["nextCursor"].clone();
    assert_eq!(
        request(
            &rt,
            &aggregate,
            Some(&ab),
            Some(json!({"cursor":row_cursor}))
        )
        .0,
        400
    );
    for input in [
        json!({"filters":{"label":{"eq":"mixed"}}}),
        json!({"limit":0}),
        json!({"limit":1.5}),
        json!({"group_by":["owner"]}),
    ] {
        assert_eq!(request(&rt, &aggregate, Some(&ab), Some(input)).0, 400);
    }
    let alias = format!("{field}(namespace:\"{namespace}\"){{visibleTotal}}");
    let (_, limited) = request(
        &rt,
        &graph,
        Some(&ab),
        Some(json!({"query":format!("{{a:{alias} b:{alias} c:{alias}}}")})),
    );
    assert!(limited.get("errors").is_some());
    assert!(limited["data"].is_null(), "{limited}");
    let row_selection = format!(
        "{}(namespace:\"{namespace}\"){{visibleTotal}}",
        model.api_name().unwrap()
    );
    let (_, mixed) = request(
        &rt,
        &graph,
        Some(&ab),
        Some(json!({"query":format!("{{a:{alias} b:{row_selection} c:{alias}}}")})),
    );
    assert!(mixed.get("errors").is_some());
    assert!(mixed["data"].is_null());
    store
        .apply_authorized(
            &plan,
            &Batch {
                context: context.clone(),
                position: "archive-b".into(),
                events: vec![Event {
                    id: "archived-b".into(),
                    contract: "b".into(),
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
    assert_eq!(request(&rt, &aggregate, Some(&a), Some(json!({}))).0, 403);
    let after_archive = request(&rt, &aggregate, Some(&ab), Some(json!({}))).1;
    let mixed_group = after_archive["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["values"]["label"] == "mixed")
        .unwrap();
    assert_eq!(mixed_group["values"]["contracts"], "1");
    assert_eq!(mixed_group["values"]["total"], "4.0000000000");
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
        request(
            &rt,
            &aggregate,
            Some(&ab),
            Some(json!({"cursor":first["nextCursor"]}))
        )
        .0,
        400
    );
    let fresh = request(&rt, &aggregate, Some(&ab), Some(json!({"limit":1}))).1;
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &aggregate, Some(&ab), Some(json!({}))).0, 503);
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &aggregate, Some(&ab), Some(json!({}))).0, 503);
    allowed.store(true, Ordering::Relaxed);
    let mut revoked = grants.clone();
    revoked["epoch"] = 2.into();
    revoked["principals"]["ab"]["enabled"] = false.into();
    std::fs::write(root.join("grants.json"), revoked.to_string()).unwrap();
    assert_eq!(request(&rt, &aggregate, Some(&ab), Some(json!({}))).0, 401);
    let mut restored = grants;
    restored["epoch"] = 3.into();
    std::fs::write(root.join("grants.json"), restored.to_string()).unwrap();
    assert_eq!(
        request(
            &rt,
            &aggregate,
            Some(&ab),
            Some(json!({"cursor":fresh["nextCursor"]}))
        )
        .0,
        400
    );
    drop(running);
    drop(cleanup);
}
