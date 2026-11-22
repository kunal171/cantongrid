//! REST/GraphQL lineage matrix against owned PostgreSQL and the common mock source authority.
use super::*;
use cantongrid_core::{
    exercise::{Exercise, ExercisePage},
    lifecycle::Origin,
    lineage::{Creation, LineagePage},
};

/// Delete only this test's stream, in foreign-key order. The server fixture owns process cleanup.
struct Cleanup(String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let Ok(mut c) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL") else {
            return;
        };
        let Ok(mut tx) = c.transaction() else { return };
        let Ok(row) = tx.query_opt(
            "SELECT stream_key FROM cantongrid.streams WHERE definition->>'source'=$1",
            &[&self.0],
        ) else {
            return;
        };
        let Some(row) = row else { return };
        let key: String = row.get(0);
        for table in [
            "lineage_outbox",
            "lineage_edges",
            "creation_events",
            "lineage_pages",
            "exercise_outbox",
            "exercise_events",
            "exercise_pages",
            "outbox",
            "events",
            "contracts",
            "batches",
            "streams",
        ] {
            if tx
                .execute(
                    &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                    &[&key],
                )
                .is_err()
            {
                return;
            }
        }
        let _ = tx.commit();
    }
}

#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton writes"]
fn direct_lineage_rest_graphql_hide_edges_before_paging_and_bind_cursors() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let id = format!("lineage-api-{}", std::process::id());
    let directory = std::env::temp_dir().join(&id);
    std::fs::create_dir(&directory).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let allowed = Arc::new(AtomicBool::new(true));
    let healthy = Arc::new(AtomicBool::new(true));
    let (url, handle) = source(stop.clone(), allowed.clone(), healthy.clone());
    let mut running = Running {
        child: None,
        directory: directory.clone(),
        stop,
        source: Some(handle),
    };
    let _cleanup = Cleanup(id.clone());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let template = TemplateId {
        package_id: "0".repeat(64),
        module: "M".into(),
        entity: "T".into(),
    };
    let context = SourceContext::new(id.clone(), "synthetic".into(), 1).unwrap();
    let source_database = "host=127.0.0.1 port=9 user=read_only dbname=synthetic";
    let plan = Plan::pqs(
        context.clone(),
        [party("A"), party("B")].into(),
        [template.clone()].into(),
    )
    .unwrap()
    .with_exercises()
    .unwrap()
    .with_lineage()
    .unwrap()
    .with_source_binding(cantongrid_runtime::config::source_binding(
        &url,
        "source-reader",
        &cantongrid_postgres::identity_local(source_database).unwrap(),
        &format!("{url}/readyz"),
    ));
    let now = Instant::now();
    let lease = SourceLease::from_verified(
        context.binding(),
        [party("A"), party("B")],
        now,
        MAX_SOURCE_LEASE,
        now + MAX_SOURCE_LEASE,
    )
    .unwrap();
    let create = |id: &str, witnesses: BTreeSet<PartyId>, snapshot: bool| Event {
        id: format!("create-{id}"),
        contract: id.into(),
        template: template.clone(),
        witnesses,
        observed_at_micros: 1,
        ledger_at_micros: None,
        origin: Some(Origin {
            offset: if snapshot { 5 } else { 10 },
            snapshot_observation: snapshot,
        }),
        action: Action::Create(Value::RawJson("{}".into())),
    };
    let mut store = Store::open_env("CANTONGRID_DESTINATION_URL").unwrap();
    let root = LineagePage {
        delivery: ExercisePage {
            batch: Batch {
                context: context.clone(),
                position: "snapshot".into(),
                events: vec![create("root", [party("A"), party("B")].into(), true)],
            },
            exercises: vec![],
        },
        creations: vec![],
    };
    store
        .apply_lineage_page(&plan, &root, &lease, None)
        .unwrap();
    let receipt = Exercise {
        version: 1,
        offset: 10,
        node_id: 0,
        transaction_id: "pair".into(),
        contract: "root".into(),
        template: template.clone(),
        choice: "Make".into(),
        consuming: false,
        argument: "{}".into(),
        result: "null".into(),
        controllers: ["operator".into()].into(),
        witnesses: ["A".into(), "B".into()].into(),
        ledger_at_micros: None,
        last_descendant_node_id: 2,
    };
    let children = [(1, "a"), (2, "b")]
        .into_iter()
        .map(|(node, contract)| Creation {
            version: 1,
            offset: 10,
            node_id: node,
            transaction_id: "pair".into(),
            contract: contract.into(),
            template: template.clone(),
            witnesses: ["A".into(), "B".into()].into(),
            divulged_only: false,
        })
        .collect();
    let page = LineagePage {
        delivery: ExercisePage {
            batch: Batch {
                context: context.clone(),
                position: "pair".into(),
                events: vec![
                    create("a", [party("A")].into(), false),
                    create("b", [party("B")].into(), false),
                ],
            },
            exercises: vec![receipt],
        },
        creations: children,
    };
    store
        .apply_lineage_page(&plan, &page, &lease, Some("snapshot"))
        .unwrap();
    let mut grants = json!({"version":1,"epoch":1,"principals":{"a":{"enabled":true,"parties":["A"]},"b":{"enabled":true,"parties":["B"]},"ab":{"enabled":true,"parties":["A","B"]}}});
    std::fs::write(directory.join("grants.json"), grants.to_string()).unwrap();
    let config = json!({"version":1,"authorization":{"endpoint":url,"user":"source-reader","source":id,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{url}/readyz"),"templates":[template],"exercise_source":"pqs041_ledger_effects","lineage_source":"pqs041_complete_subtrees","history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":address,"issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
    std::fs::write(directory.join("runtime.json"), config.to_string()).unwrap();
    let key = "local-lineage-organization-test-key-00001";
    running.child = Some(
        Command::new(env!("CARGO_BIN_EXE_cantongrid"))
            .args(["serve", directory.join("runtime.json").to_str().unwrap()])
            .env("CANTONGRID_TEST_PQS_URL", source_database)
            .env(
                "CANTONGRID_TEST_SOURCE_TOKEN",
                jwt("source-reader", key, clock() + 600),
            )
            .env("CANTONGRID_TEST_API_KEY", key)
            .env(
                "CANTONGRID_TEST_CURSOR_KEY",
                "local-lineage-cursor-test-key-00000001",
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
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{address}");
    let a = jwt("a", key, clock() + 600);
    let b = jwt("b", key, clock() + 600);
    let ab = jwt("ab", key, clock() + 600);
    let path = format!("{base}/v1/contracts/root/lineage?direction=children&limit=1");
    assert_eq!(request(&rt, &path, None, None).0, 401);
    for (token, child) in [(&a, "a"), (&b, "b")] {
        let (status, result) = request(&rt, &path, Some(token), None);
        assert_eq!(status, 200);
        assert_eq!(result["visibleTotal"], "1");
        assert!(result["nextCursor"].is_null());
        assert_eq!(result["items"][0]["childContractId"], child);
        assert_eq!(result["provenance"]["complete"], false);
    }
    let (_, first) = request(&rt, &path, Some(&ab), None);
    assert_eq!(first["visibleTotal"], "2");
    let cursor = first["nextCursor"].as_str().unwrap();
    let (_, second) = request(&rt, &format!("{path}&cursor={cursor}"), Some(&ab), None);
    assert_ne!(first["items"][0], second["items"][0]);
    assert!(second["nextCursor"].is_null());
    for wrong in [
        format!("{base}/v1/contracts/a/lineage?direction=children&cursor={cursor}"),
        format!("{base}/v1/contracts/root/lineage?direction=parents&cursor={cursor}"),
    ] {
        assert_eq!(request(&rt, &wrong, Some(&ab), None).0, 400);
    }
    assert_eq!(
        request(&rt, &format!("{path}&cursor={cursor}"), Some(&a), None).0,
        400
    );
    let hidden = request(
        &rt,
        &format!("{base}/v1/contracts/b/lineage?direction=parents"),
        Some(&a),
        None,
    );
    let unknown = request(
        &rt,
        &format!("{base}/v1/contracts/unknown/lineage?direction=parents"),
        Some(&a),
        None,
    );
    assert_eq!(hidden, unknown);
    assert_eq!(hidden.0, 404);
    let query = r#"
        query Lineage($root: String!, $direction: String!, $limit: Int) {
            contractLineage(contractId: $root, direction: $direction, limit: $limit) {
                items
                visibleTotal
                nextCursor
                provenance
            }
        }
    "#;
    let (_, graph) = request(
        &rt,
        &base,
        Some(&a),
        Some(json!({"query":query,"variables":{"root":"root","direction":"children","limit":1}})),
    );
    assert!(graph.get("errors").is_none());
    assert_eq!(graph["data"]["contractLineage"]["visibleTotal"], "1");
    grants["epoch"] = 2.into();
    std::fs::write(directory.join("grants.json"), grants.to_string()).unwrap();
    assert_eq!(
        request(&rt, &format!("{path}&cursor={cursor}"), Some(&ab), None).0,
        400
    );
    grants["principals"]["a"]["enabled"] = false.into();
    std::fs::write(directory.join("grants.json"), grants.to_string()).unwrap();
    assert_eq!(request(&rt, &path, Some(&a), None).0, 401);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &path, Some(&ab), None).0, 503);
    allowed.store(true, Ordering::Relaxed);
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &path, Some(&ab), None).0, 503);
}
