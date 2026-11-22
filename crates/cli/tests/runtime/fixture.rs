//! Shared owned-database/mock-source harness for temporal, durable delivery and hardening checks.
use super::*;
use cantongrid_core::lifecycle::Origin;
use std::sync::atomic::AtomicU64;
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Own one server, source scope and destination stream. No helper writes to a Canton ledger.
pub(super) struct Fixture {
    pub running: Running,
    pub allowed: Arc<AtomicBool>,
    pub healthy: Arc<AtomicBool>,
    pub plan: Plan,
    pub template: TemplateId,
    pub store: Store,
    pub rt: tokio::runtime::Runtime,
    pub base: String,
    key: String,
}
impl Fixture {
    /// Start the common source-rights stub and a real API child on separate ephemeral ports.
    pub fn new() -> Self {
        initialize_env("CANTONGRID_DESTINATION_URL").unwrap();
        let id = format!(
            "extended-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
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
            context,
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
        let grants = json!({"version":1,"epoch":1,"principals":{"a":{"enabled":true,"parties":["A"]},"b":{"enabled":true,"parties":["B"]},"ab":{"enabled":true,"parties":["A","B"]}}});
        std::fs::write(directory.join("grants.json"), grants.to_string()).unwrap();
        let config = json!({"version":1,"authorization":{"endpoint":url,"user":"source-reader","source":id,"network":"synthetic","generation":1,"parties":["A","B"],"credential_env":"CANTONGRID_TEST_SOURCE_TOKEN"},"source_database_env":"CANTONGRID_TEST_PQS_URL","destination_database_env":"CANTONGRID_DESTINATION_URL","source_health":format!("{url}/readyz"),"templates":[template],"history":"snapshot_forward_partial","archive_policy":"prior_create_current_grants","payload":"raw_json","retention":"retain_while_authorized","api":{"bind":address,"issuer":"owned-tests","audience":"owned-api","token_key_env":"CANTONGRID_TEST_API_KEY","cursor_key_env":"CANTONGRID_TEST_CURSOR_KEY","grants_file":"grants.json"}});
        std::fs::write(directory.join("runtime.json"), config.to_string()).unwrap();
        let key = "local-extended-organization-test-key-00001".to_string();
        running.child = Some(
            Command::new(env!("CARGO_BIN_EXE_cantongrid"))
                .args(["serve", directory.join("runtime.json").to_str().unwrap()])
                .env("CANTONGRID_TEST_PQS_URL", source_database)
                .env(
                    "CANTONGRID_TEST_SOURCE_TOKEN",
                    jwt("source-reader", &key, clock() + 600),
                )
                .env("CANTONGRID_TEST_API_KEY", &key)
                .env(
                    "CANTONGRID_TEST_CURSOR_KEY",
                    "local-extended-cursor-test-key-00000001",
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
        Self {
            running,
            allowed,
            healthy,
            plan,
            template,
            store: Store::open_env("CANTONGRID_DESTINATION_URL").unwrap(),
            rt,
            base: format!("http://{address}"),
            key,
        }
    }
    /// Restart only the API child, preserving the source stub, configuration and durable database.
    pub fn restart(&mut self) {
        if let Some(mut child) = self.running.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.running.child = Some(
            Command::new(env!("CARGO_BIN_EXE_cantongrid"))
                .args([
                    "serve",
                    self.running
                        .directory
                        .join("runtime.json")
                        .to_str()
                        .unwrap(),
                ])
                .env(
                    "CANTONGRID_TEST_PQS_URL",
                    "host=127.0.0.1 port=9 user=read_only dbname=synthetic",
                )
                .env(
                    "CANTONGRID_TEST_SOURCE_TOKEN",
                    jwt("source-reader", &self.key, clock() + 600),
                )
                .env("CANTONGRID_TEST_API_KEY", &self.key)
                .env(
                    "CANTONGRID_TEST_CURSOR_KEY",
                    "local-extended-cursor-test-key-00000001",
                )
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let address = self.base.strip_prefix("http://").unwrap();
        for _ in 0..100 {
            if std::net::TcpStream::connect(address).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("API child did not restart within fixture deadline");
    }
    /// Fresh synthetic destination-write authority; API reads independently consult the source stub.
    pub fn lease(&self) -> SourceLease {
        let now = Instant::now();
        SourceLease::from_verified(
            self.plan.context().binding(),
            [party("A"), party("B")],
            now,
            MAX_SOURCE_LEASE,
            now + MAX_SOURCE_LEASE,
        )
        .unwrap()
    }
    /// An explicitly scoped canonical fixture; exact values are transported as strings.
    pub fn create(&self, id: &str, witness: &str, offset: i64, snapshot: bool) -> Event {
        Event {
            id: format!("create-{id}"),
            contract: id.into(),
            template: self.template.clone(),
            witnesses: [party(witness)].into(),
            observed_at_micros: 1,
            ledger_at_micros: Some(1),
            origin: Some(Origin {
                offset,
                snapshot_observation: snapshot,
            }),
            action: Action::Create(Value::RawJson(
                r#"{"amount":"12345678901234567890.0000000001","quantity":"9223372036854775807"}"#
                    .into(),
            )),
        }
    }
    /// Commit a complete supported interval, including empty interval/snapshot acknowledgements.
    pub fn commit(&mut self, boundary: i64, events: Vec<Event>) {
        let batch=Batch {context:self.plan.context().clone(),position:json!({"snapshot":false,"boundary":boundary,"through":boundary,"offset":boundary,"pk":i64::MAX}).to_string(),events};
        self.store
            .apply_authorized(&self.plan, &batch, &self.lease())
            .unwrap();
    }
    /// Obtain an independent organization bearer; no API method accepts a party list as authority.
    pub fn token(&self, subject: &str) -> String {
        jwt(subject, &self.key, clock() + 600)
    }
    /// Reuse the common HTTP client and its bounded response parser.
    pub fn get(&self, path: &str, subject: &str) -> (u16, Json) {
        request(
            &self.rt,
            &format!("{}{path}", self.base),
            Some(&self.token(subject)),
            None,
        )
    }
    /// Send a JSON operation to the actual API process.
    pub fn post(&self, path: &str, subject: &str, body: Json) -> (u16, Json) {
        self.rt.block_on(async {
            let response = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap()
                .post(format!("{}{path}", self.base))
                .bearer_auth(self.token(subject))
                .header("content-type", "application/json")
                .body(body.to_string())
                .send()
                .await
                .unwrap();
            let status = response.status().as_u16();
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            let bytes = response.bytes().await.unwrap();
            assert!(bytes.len() <= 2 * 1024 * 1024);
            (status, serde_json::from_slice(&bytes).unwrap())
        })
    }
    /// Atomically replace the local grant document; an epoch change invalidates existing cursors.
    pub fn revoke_a(&self) {
        std::fs::write(self.running.directory.join("grants.json"),json!({"version":1,"epoch":2,"principals":{"b":{"enabled":true,"parties":["B"]},"ab":{"enabled":true,"parties":["A","B"]}}}).to_string()).unwrap();
    }
}
impl Drop for Fixture {
    /// Remove only this source's owned destination rows. Future subscription rows cascade on stream deletion.
    fn drop(&mut self) {
        let (source, _, _) = self.plan.context().identity();
        let Ok(mut client) = cantongrid_postgres::connect_env("CANTONGRID_DESTINATION_URL") else {
            return;
        };
        let Ok(mut tx) = client.transaction() else {
            return;
        };
        let Ok(Some(row)) = tx.query_opt(
            "SELECT stream_key FROM cantongrid.streams WHERE definition->>'source'=$1",
            &[&source],
        ) else {
            return;
        };
        let key: String = row.get(0);
        for table in ["outbox", "events", "contracts", "batches", "streams"] {
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
