//! Owned-Postgres end-to-end privacy controls with a synthetic read-only source authority.
//! No Canton party, ledger transaction or upstream database mutation occurs in this test.
use cantongrid_core::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    config::{Plan, SourceContext, TemplateId},
    lifecycle::{Action, Batch, Event},
    value::Value,
    visibility::PartyId,
};
use cantongrid_storage::{Store, initialize_env};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Running {
    child: Option<Child>,
    directory: PathBuf,
    stop: Arc<AtomicBool>,
    source: Option<thread::JoinHandle<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.source.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn party(s: &str) -> PartyId {
    PartyId::new(s).unwrap()
}
fn clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
fn jwt(subject: &str, key: &str, expires: u64) -> String {
    encode(&Header::new(Algorithm::HS256),&json!({"sub":subject,"iss":"owned-tests","aud":"owned-api","exp":expires,"iat":clock().saturating_sub(10)}),&EncodingKey::from_secret(key.as_bytes()).unwrap()).unwrap()
}

/// A finite-memory fake source supports only authenticated GET user/rights and readiness.
/// Atomic switches simulate loss of permission/readiness without changing any Canton grants.
fn source(
    stop: Arc<AtomicBool>,
    allowed: Arc<AtomicBool>,
    healthy: Arc<AtomicBool>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.windows(4).any(|s| s == b"\r\n\r\n") || bytes.len() > 16384 {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&bytes);
            let head = request.lines().next().unwrap_or("");
            let (status, body) = if head.starts_with("GET /readyz ") {
                (
                    200,
                    json!({"status":"ok","grpc_up":healthy.load(Ordering::Relaxed),"jdbc_connection_pool_up":true,"stream_up":true}),
                )
            } else if !allowed.load(Ordering::Relaxed) {
                (403, json!({}))
            } else if head.starts_with("GET /v2/users/source-reader/rights ") {
                (
                    200,
                    json!({"rights":[{"kind":{"CanReadAs":{"value":{"party":"A"}}}},{"kind":{"CanReadAs":{"value":{"party":"B"}}}}]}),
                )
            } else if head.starts_with("GET /v2/users/source-reader ") {
                (
                    200,
                    json!({"user":{"id":"source-reader","isDeactivated":false,"identityProviderId":""}}),
                )
            } else {
                (404, json!({}))
            };
            let body = body.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (address, handle)
}

fn request(
    rt: &tokio::runtime::Runtime,
    address: &str,
    token: Option<&str>,
    query: Option<Json>,
) -> (u16, Json) {
    rt.block_on(async {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let mut request = if let Some(body) = query {
            client
                .post(format!("{address}/graphql"))
                .header("content-type", "application/json")
                .body(body.to_string())
        } else {
            client.get(address)
        };
        if let Some(token) = token {
            request = request.bearer_auth(token)
        }
        let mut response = request.send().await.unwrap();
        let status = response.status().as_u16();
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.unwrap() {
            bytes.extend_from_slice(&chunk)
        }
        (status, serde_json::from_slice(&bytes).unwrap())
    })
}

#[test]
#[ignore = "requires the explicitly initialized owned destination database"]
fn rest_graphql_enforce_party_scopes_cursors_revocation_and_source_outages() {
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let root = std::env::temp_dir().join(format!("cantongrid-runtime-test-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let allowed = Arc::new(AtomicBool::new(true));
    let healthy = Arc::new(AtomicBool::new(true));
    let (source_url, thread) = source(stop.clone(), allowed.clone(), healthy.clone());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let context = SourceContext::new(
        format!("owned-runtime-{}", std::process::id()),
        "synthetic".into(),
        1,
    )
    .unwrap();
    let template = TemplateId {
        package_id: "0".repeat(64),
        module: "Unrelated".into(),
        entity: "Record".into(),
    };
    let plan = Plan::pqs(
        context.clone(),
        BTreeSet::from([party("A"), party("B")]),
        BTreeSet::from([template.clone()]),
    )
    .unwrap();
    let source_database = "host=127.0.0.1 port=9 user=read_only dbname=synthetic";
    let plan = plan.with_source_binding(cantongrid_runtime::config::source_binding(
        &source_url,
        "source-reader",
        &cantongrid_postgres::identity_local(source_database).unwrap(),
        &format!("{source_url}/readyz"),
    ));
    let mut store = Store::open_env("CANTONGRID_DESTINATION_URL").unwrap();
    let time = Instant::now();
    let lease = SourceLease::from_verified(
        context.binding(),
        [party("A"), party("B")],
        time,
        MAX_SOURCE_LEASE,
        time + Duration::from_secs(60),
    )
    .unwrap();
    let events=[("a1",vec![party("A")]),("a2",vec![party("A")]),("b1",vec![party("B")]),("joint",vec![party("A"),party("B")])].into_iter().map(|(id,witnesses)|Event{id:format!("create-{id}"),contract:id.into(),template:template.clone(),witnesses:witnesses.into_iter().collect(),observed_at_micros:1,ledger_at_micros:Some(1),origin:None,action:Action::Create(Value::RawJson(json!({"label":id,"amount":"12345678901234567890.1234567891","sequence":"9223372036854775807"}).to_string()))}).collect();
    let position = |n: i64| {
        json!({"snapshot":false,"boundary":n,"through":n,"offset":n,"pk":i64::MAX}).to_string()
    };
    let batch = Batch {
        context: context.clone(),
        position: position(10),
        events,
    };
    assert!(store.apply_authorized(&plan, &batch, &lease).unwrap());
    assert!(!store.apply_authorized(&plan, &batch, &lease).unwrap());
    let archive = Batch {
        context: context.clone(),
        position: position(20),
        events: vec![Event {
            id: "archive-a2".into(),
            contract: "a2".into(),
            template: template.clone(),
            witnesses: BTreeSet::new(),
            observed_at_micros: 2,
            ledger_at_micros: Some(2),
            origin: None,
            action: Action::ArchiveByPriorContract,
        }],
    };
    store.apply_authorized(&plan, &archive, &lease).unwrap();
    assert_eq!(
        store.checkpoint(&plan).unwrap().as_deref(),
        Some(position(20).as_str())
    );
    // Live progress must remain functional beyond the fixture reference's 256-batch journal cap.
    for n in 21..280 {
        store
            .apply_authorized(
                &plan,
                &Batch {
                    context: context.clone(),
                    position: position(n),
                    events: vec![],
                },
                &lease,
            )
            .unwrap();
    }
    // Stage a valid creation and then an orphan archive in the same page. The entire
    // transaction must roll back, including the first row, journal, outbox and checkpoint.
    let checkpoint_before = store.checkpoint(&plan).unwrap();
    let mut staged = batch.events[0].clone();
    staged.id = "rolled-back-create".into();
    staged.contract = "rolled-back".into();
    let mut orphan = archive.events[0].clone();
    orphan.id = "orphan-archive".into();
    orphan.contract = "never-observed".into();
    assert!(
        store
            .apply_authorized(
                &plan,
                &Batch {
                    context: context.clone(),
                    position: position(300),
                    events: vec![staged, orphan]
                },
                &lease
            )
            .is_err()
    );
    assert_eq!(store.checkpoint(&plan).unwrap(), checkpoint_before);
    // An actual SQL uniqueness failure after staging a new contract must roll it back
    // together with any journal/progress/outbox effects, not leave an orphan projection.
    let mut conflicting = batch.events[0].clone();
    conflicting.contract = "must-rollback-after-sql-error".into();
    assert!(
        store
            .apply_authorized(
                &plan,
                &Batch {
                    context: context.clone(),
                    position: position(303),
                    events: vec![conflicting]
                },
                &lease
            )
            .is_err()
    );
    assert_eq!(store.checkpoint(&plan).unwrap(), checkpoint_before);
    // A stale worker cannot rewind progress after another worker has committed a later page.
    assert!(
        store
            .apply_page(
                &plan,
                &Batch {
                    context: context.clone(),
                    position: position(302),
                    events: vec![]
                },
                &lease,
                Some(&position(10))
            )
            .is_err()
    );
    assert_eq!(store.checkpoint(&plan).unwrap(), checkpoint_before);
    // Narrowed/invalidated source authorization cannot publish a cached page.
    let mut revoked = SourceLease::from_verified(
        context.binding(),
        [party("A"), party("B")],
        Instant::now(),
        MAX_SOURCE_LEASE,
        Instant::now() + Duration::from_secs(60),
    )
    .unwrap();
    revoked.invalidate();
    assert!(
        store
            .apply_authorized(
                &plan,
                &Batch {
                    context: context.clone(),
                    position: position(301),
                    events: vec![]
                },
                &revoked
            )
            .is_err()
    );
    assert_eq!(store.checkpoint(&plan).unwrap(), checkpoint_before);
    let mut grants = json!({"version":1,"epoch":1,"principals":{"a":{"enabled":true,"parties":["A"]},"b":{"enabled":true,"parties":["B"]},"ab":{"enabled":true,"parties":["A","B"]}}});
    let grants_path = root.join("grants.json");
    std::fs::write(&grants_path, grants.to_string()).unwrap();
    let config = json!({"version":1,"authorization":{"endpoint":source_url,"user":"source-reader","source":context.identity().0,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{source_url}/readyz"),"templates":[template],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":address,"issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
    let path = root.join("runtime.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let key = "local-owned-fixture-organization-key-0001";
    let cursor_key = "local-owned-fixture-cursor-key-00000001";
    // Source signatures are checked by the real Ledger in LocalNet tests; this fake only
    // isolates transport privacy. No fake source credential is used in a deployed runtime.
    let source_token = jwt("source-reader", key, clock() + 600);
    let child = Command::new(env!("CARGO_BIN_EXE_cantongrid"))
        .args(["serve", path.to_str().unwrap()])
        .env("CANTONGRID_TEST_PQS_URL", source_database)
        .env("CANTONGRID_TEST_SOURCE_TOKEN", source_token)
        .env("CANTONGRID_TEST_API_KEY", key)
        .env("CANTONGRID_TEST_CURSOR_KEY", cursor_key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _running = Running {
        child: Some(child),
        directory: root,
        stop,
        source: Some(thread),
    };
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
    assert_eq!(
        request(&rt, &format!("{base}/v1/contracts"), None, None).0,
        401
    );
    let a = jwt("a", key, clock() + 600);
    let b = jwt("b", key, clock() + 600);
    let ab = jwt("ab", key, clock() + 600);
    for (token, active, history) in [(&a, "2", "4"), (&b, "2", "2"), (&ab, "3", "5")] {
        let (status, result) = request(&rt, &format!("{base}/v1/contracts"), Some(token), None);
        assert_eq!(status, 200);
        assert_eq!(result["visibleTotal"], active);
        let expected: BTreeSet<&str> = if token == &a {
            BTreeSet::from(["a1", "joint"])
        } else if token == &b {
            BTreeSet::from(["b1", "joint"])
        } else {
            BTreeSet::from(["a1", "b1", "joint"])
        };
        let ids: BTreeSet<_> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["contractId"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected);
        assert_eq!(result["provenance"]["coverage"], "partial_history");
        let encoded = result.to_string();
        assert!(encoded.contains("12345678901234567890.1234567891"));
        assert!(encoded.contains("9223372036854775807"));
        let (status, result) = request(&rt, &format!("{base}/v1/events"), Some(token), None);
        assert_eq!(status, 200);
        assert_eq!(result["visibleTotal"], history);
        for item in result["items"].as_array().unwrap() {
            assert!(item.get("envelope").is_none());
            assert!(item.get("position").is_none());
            assert!(item.get("witnesses").is_none());
        }
        let (status, result) = request(
            &rt,
            &base,
            Some(token),
            Some(
                json!({"query":"{ activeContracts { visibleTotal items provenance } lifecycleEvents { visibleTotal items } }"}),
            ),
        );
        assert_eq!(status, 200);
        assert!(result.get("errors").is_none(), "GraphQL operation failed");
        assert_eq!(result["data"]["activeContracts"]["visibleTotal"], active);
        let ids: BTreeSet<_> = result["data"]["activeContracts"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["contractId"].as_str().unwrap())
            .collect();
        assert_eq!(ids, expected);
        assert_eq!(result["data"]["lifecycleEvents"]["visibleTotal"], history);
    }
    assert_eq!(
        request(
            &rt,
            &format!("{base}/v1/contracts"),
            Some(&jwt(
                "a",
                "different-organization-signing-key-0001",
                clock() + 600
            )),
            None
        )
        .0,
        401
    );
    for (issuer, audience) in [
        ("wrong-issuer", "owned-api"),
        ("owned-tests", "wrong-audience"),
    ] {
        let invalid = encode(
            &Header::new(Algorithm::HS256),
            &json!({"sub":"a","iss":issuer,"aud":audience,"iat":clock(),"exp":clock()+600}),
            &EncodingKey::from_secret(key.as_bytes()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            request(
                &rt,
                &base,
                Some(&invalid),
                Some(json!({"query":"{ activeContracts { items } }"}))
            )
            .0,
            401
        );
    }
    let (_, first) = request(&rt, &format!("{base}/v1/contracts?limit=1"), Some(&a), None);
    let cursor = first["nextCursor"].as_str().unwrap();
    let (_, second) = request(
        &rt,
        &format!("{base}/v1/contracts?limit=1&cursor={cursor}"),
        Some(&a),
        None,
    );
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_ne!(first["items"][0], second["items"][0]);
    assert_ne!(
        request(
            &rt,
            &format!("{base}/v1/contracts?cursor={cursor}"),
            Some(&b),
            None
        )
        .0,
        200
    );
    assert_ne!(
        request(
            &rt,
            &format!("{base}/v1/events?cursor={cursor}"),
            Some(&a),
            None
        )
        .0,
        200
    );
    assert_ne!(
        request(
            &rt,
            &format!("{base}/v1/contracts?cursor={cursor}x"),
            Some(&a),
            None
        )
        .0,
        200
    );
    assert_eq!(
        request(
            &rt,
            &format!("{base}/v1/contracts"),
            Some(&jwt("a", key, clock() - 1)),
            None
        )
        .0,
        401
    );
    grants["principals"]["a"]["enabled"] = false.into();
    std::fs::write(&grants_path, grants.to_string()).unwrap();
    assert_eq!(
        request(&rt, &format!("{base}/v1/contracts"), Some(&a), None).0,
        401
    );
    assert_eq!(
        request(
            &rt,
            &base,
            Some(&a),
            Some(json!({"query":"{ activeContracts { items } }"}))
        )
        .0,
        401
    );
    grants["principals"]["a"]["enabled"] = true.into();
    grants["epoch"] = 2.into();
    std::fs::write(&grants_path, grants.to_string()).unwrap();
    assert_ne!(
        request(
            &rt,
            &format!("{base}/v1/contracts?cursor={cursor}"),
            Some(&a),
            None
        )
        .0,
        200
    );
    healthy.store(false, Ordering::Relaxed);
    assert_eq!(
        request(&rt, &format!("{base}/v1/contracts"), Some(&a), None).0,
        503
    );
    healthy.store(true, Ordering::Relaxed);
    allowed.store(false, Ordering::Relaxed);
    assert_eq!(
        request(&rt, &format!("{base}/v1/contracts"), Some(&a), None).0,
        503
    );
    assert_eq!(
        request(
            &rt,
            &base,
            Some(&a),
            Some(json!({"query":"{ activeContracts { items } }"}))
        )
        .0,
        503
    );
    // Remove only this test's destination stream, never another test or source database.
    let mut db = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    let source = context.identity().0;
    let mut tx = db.transaction().unwrap();
    for table in ["outbox", "events", "contracts", "batches", "streams"] {
        tx.execute(&format!("DELETE FROM cantongrid.{table} WHERE stream_key IN (SELECT stream_key FROM cantongrid.streams WHERE definition->>'source'=$1)"),&[&source]).unwrap();
    }
    tx.commit().unwrap();
}

#[test]
#[ignore = "requires owned PostgreSQL; mock source authority; no Canton ledger writes"]
fn model_binding_cli_checks_source_authority_and_readiness_before_activation() {
    use cantongrid_models::Catalog;
    initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
    let root = std::env::temp_dir().join(format!("cantongrid-bind-cli-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let allowed = Arc::new(AtomicBool::new(true));
    let healthy = Arc::new(AtomicBool::new(true));
    let (url, source_thread) = source(stop.clone(), allowed.clone(), healthy.clone());
    let _running = Running {
        child: None,
        directory: root.clone(),
        stop,
        source: Some(source_thread),
    };
    let namespace = format!("bind_cli_{}", std::process::id());
    let mut definition: Json =
        serde_json::from_str(include_str!("../assets/models/visibility.json")).unwrap();
    definition["models"][0]["name"] = namespace.clone().into();
    let catalog = Catalog::parse(&definition.to_string()).unwrap();
    let model = &catalog.models()[0];
    let template = &model.template;
    let context = SourceContext::new(namespace.clone(), "synthetic".into(), 1).unwrap();
    let source_database = "host=127.0.0.1 port=9 user=read_only dbname=synthetic";
    let plan = Plan::pqs(
        context.clone(),
        [party("A"), party("B")].into(),
        [template.clone()].into(),
    )
    .unwrap()
    .with_source_binding(cantongrid_runtime::config::source_binding(
        &url,
        "source-reader",
        &cantongrid_postgres::identity_local(source_database).unwrap(),
        &format!("{url}/readyz"),
    ));
    let config = json!({"version":1,"authorization":{"endpoint":url,"user":"source-reader","source":namespace,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{url}/readyz"),"templates":[template],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":"127.0.0.1:0","issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
    let path = root.join("runtime.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let key = "local-owned-fixture-organization-key-0001";
    let token = jwt("source-reader", key, clock() + 600);
    let command = |rest: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_cantongrid"));
        cmd.args([
            "models",
            "bind",
            path.to_str().unwrap(),
            &namespace,
            &model.name,
            "1",
        ])
        .args(rest)
        .env("CANTONGRID_TEST_PQS_URL", source_database)
        .env("CANTONGRID_TEST_SOURCE_TOKEN", &token)
        .env_remove("CANTONGRID_TEST_API_KEY")
        .env_remove("CANTONGRID_TEST_CURSOR_KEY");
        cmd.output().unwrap()
    };
    let mut store = Store::open_env("CANTONGRID_DESTINATION_URL").unwrap();
    store.register_models(&namespace, &catalog).unwrap();
    store.install_model(&namespace, &model.name, 1).unwrap();
    let lease = SourceLease::from_verified(
        context.binding(),
        [party("A"), party("B")],
        Instant::now(),
        MAX_SOURCE_LEASE,
        Instant::now() + Duration::from_secs(60),
    )
    .unwrap();
    store
        .apply_authorized(
            &plan,
            &Batch {
                context,
                position: "snapshot".into(),
                events: vec![Event {
                    id: "created".into(),
                    contract: "first".into(),
                    template: template.clone(),
                    witnesses: [party("A")].into(),
                    observed_at_micros: 1,
                    ledger_at_micros: None,
                    origin: None,
                    action: Action::Create(Value::RawJson(
                        include_str!("../assets/models/visibility-payload.json").into(),
                    )),
                }],
            },
            &lease,
        )
        .unwrap();
    let relation = format!("cantongrid_models.\"{}\"", model.sql_plan().unwrap().table);
    let count = || -> i64 {
        cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL")
            .unwrap()
            .query_one(&format!("SELECT count(*) FROM {relation}"), &[])
            .unwrap()
            .get(0)
    };
    for (allow, health) in [(false, true), (true, false)] {
        allowed.store(allow, Ordering::Relaxed);
        healthy.store(health, Ordering::Relaxed);
        let output = command(&[]);
        assert!(!output.status.success());
        assert_eq!(count(), 0);
        assert!(!String::from_utf8_lossy(&output.stderr).contains(&token));
    }
    allowed.store(true, Ordering::Relaxed);
    healthy.store(true, Ordering::Relaxed);
    assert!(!command(&["--replace", "0"]).status.success());
    let output = command(&[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["changed"], true);
    assert_eq!(result["result"]["projected_rows"], 1);
    assert_eq!(count(), 1);
    let retry: Json = serde_json::from_slice(&command(&[]).stdout).unwrap();
    assert_eq!(retry["result"]["changed"], false);
    assert_eq!(
        store.checkpoint(&plan).unwrap().as_deref(),
        Some("snapshot")
    );
    // Only test-owned destination rows/tables are removed; the fake authority is read-only.
    let mut client = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL").unwrap();
    let stream: String = client
        .query_one(
            "SELECT stream_key FROM cantongrid.model_bindings WHERE namespace=$1",
            &[&namespace],
        )
        .unwrap()
        .get(0);
    client
        .batch_execute(&format!("DROP TABLE {relation}"))
        .unwrap();
    for table in [
        "model_bindings",
        "outbox",
        "events",
        "contracts",
        "batches",
        "streams",
    ] {
        client
            .execute(
                &format!("DELETE FROM cantongrid.{table} WHERE stream_key=$1"),
                &[&stream],
            )
            .unwrap();
    }
    client
        .execute(
            "DELETE FROM cantongrid.model_installations WHERE namespace=$1",
            &[&namespace],
        )
        .unwrap();
    client
        .execute(
            "DELETE FROM cantongrid.model_revisions WHERE namespace=$1",
            &[&namespace],
        )
        .unwrap();
    client
        .execute(
            "DELETE FROM cantongrid.model_tables WHERE fingerprint=$1",
            &[&model.fingerprint().unwrap()],
        )
        .unwrap();
}
