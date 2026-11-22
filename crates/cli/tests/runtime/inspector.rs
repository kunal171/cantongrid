//! REST/GraphQL inspector matrix against owned PostgreSQL and the common mock source authority.
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
fn transaction_inspector_hides_nodes_and_binds_visible_ordinal_cursors() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let id = format!("inspect-api-{}", std::process::id());
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
        argument: "{\"secret-sentinel\":true}".into(),
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
    let private = LineagePage {
        delivery: ExercisePage {
            batch: Batch {
                context: context.clone(),
                position: "private".into(),
                events: vec![],
            },
            exercises: vec![Exercise {
                version: 1,
                offset: 20,
                node_id: 0,
                transaction_id: "private-tx".into(),
                contract: "b".into(),
                template: template.clone(),
                choice: "Private".into(),
                consuming: false,
                argument: "{}".into(),
                result: "null".into(),
                controllers: ["secret-controller".into()].into(),
                witnesses: ["A".into(), "B".into()].into(),
                ledger_at_micros: None,
                last_descendant_node_id: 0,
            }],
        },
        creations: vec![],
    };
    store
        .apply_lineage_page(&plan, &private, &lease, Some("pair"))
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
    let url = format!("{base}/v1/inspect/transactions/pair");
    assert_eq!(request(&rt, &url, None, None).0, 401);
    assert_eq!(
        request(&rt, &url, Some(&jwt("ungranted", key, clock() + 600)), None).0,
        401
    );
    for (token, total) in [(&a, "2"), (&b, "2"), (&ab, "3")] {
        let result = request(&rt, &url, Some(token), None).1;
        assert_eq!(result["visibleTotal"], total);
        assert_eq!(
            result["provenance"]["coverage"],
            "partial_recorded_evidence"
        );
        for (index, item) in result["items"].as_array().unwrap().iter().enumerate() {
            assert_eq!(item["ordinal"], (index + 1).to_string());
        }
        let text = result.to_string();
        for forbidden in [
            "secret-",
            "node_id",
            "last_descendant",
            "controllers",
            "witnesses",
            "argument",
            "sourceLocalWatermark",
        ] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    }
    let unknown = request(
        &rt,
        &format!("{base}/v1/inspect/transactions/missing"),
        Some(&a),
        None,
    );
    let hidden = request(
        &rt,
        &format!("{base}/v1/inspect/transactions/private-tx"),
        Some(&a),
        None,
    );
    assert_eq!(unknown, hidden);
    assert_eq!(hidden.0, 404);
    assert_eq!(
        request(
            &rt,
            &format!("{base}/v1/inspect/transactions/private-tx"),
            Some(&b),
            None
        )
        .1["visibleTotal"],
        "1"
    );
    let first = request(&rt, &format!("{url}?limit=1"), Some(&ab), None).1;
    let operation = json!({"query":"query($cursor:String){inspectTransaction(transactionId:\"pair\",limit:1,cursor:$cursor){items visibleTotal nextCursor provenance}}","variables":{"cursor":first["nextCursor"]}});
    let gql = request(&rt, &base, Some(&ab), Some(operation)).1;
    assert!(gql.get("errors").is_none(), "{gql}");
    let cursor = first["nextCursor"].as_str().unwrap();
    let second = request(
        &rt,
        &format!("{url}?limit=1&cursor={cursor}"),
        Some(&ab),
        None,
    )
    .1;
    assert_eq!(gql["data"]["inspectTransaction"]["items"], second["items"]);
    for target in [
        format!("{url}?cursor={cursor}"),
        format!("{base}/v1/inspect/transactions/private-tx?cursor={cursor}"),
        format!("{base}/v1/contracts?cursor={cursor}"),
    ] {
        let token = if target.starts_with(&format!("{url}?")) {
            &a
        } else {
            &ab
        };
        assert_eq!(request(&rt, &target, Some(token), None).0, 400);
    }
    assert_eq!(
        request(&rt, &format!("{url}?limit=26"), Some(&a), None).0,
        400
    );
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &url, Some(&a), None).0, 503);
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(request(&rt, &url, Some(&a), None).0, 503);
    allowed.store(true, Ordering::Relaxed);
    grants["epoch"] = 2.into();
    grants["principals"]["ab"]["enabled"] = false.into();
    std::fs::write(directory.join("grants.json"), grants.to_string()).unwrap();
    assert_eq!(request(&rt, &url, Some(&ab), None).0, 401);
}
