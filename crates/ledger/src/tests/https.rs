//! Real loopback TLS controls use an ephemeral test certificate. Python/OpenSSL are development
//! helpers only; the product transport uses the approved Rustls dependency graph.
use super::*;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Bound fixture lifecycle; child processes and private temporary keys are removed even after assertion failure.
struct Fixture {
    directory: PathBuf,
    child: Child,
    port: u16,
    certificate: Vec<u8>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl Fixture {
    /// Generate a one-day localhost-only certificate and start an isolated synthetic Ledger responder.
    fn start() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "cantongrid-tls-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let status = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                "ca.key",
                "-out",
                "ca.pem",
                "-days",
                "1",
                "-subj",
                "/CN=TestCA",
            ])
            .current_dir(&directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("TLS tests require system openssl");
        assert!(status.success());
        let status = Command::new("openssl")
            .args([
                "req",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
                "-subj",
                "/CN=localhost",
            ])
            .current_dir(&directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("TLS tests require system openssl");
        assert!(status.success());
        std::fs::write(
            directory.join("ext.cnf"),
            b"subjectAltName=DNS:localhost\nbasicConstraints=CA:FALSE\n",
        )
        .unwrap();
        let status = Command::new("openssl")
            .args([
                "x509",
                "-req",
                "-in",
                "server.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-out",
                "server.pem",
                "-days",
                "1",
                "-extfile",
                "ext.cnf",
            ])
            .current_dir(&directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("TLS tests require system openssl");
        assert!(status.success());
        let script = r#"
import http.server,ssl,json,os
class Handler(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_GET(self):
  if not self.headers.get('Authorization','').startswith('Bearer '):
   self.send_response(401); self.end_headers(); return
  if os.path.exists('redirect'):
   self.send_response(302); self.send_header('Location','http://127.0.0.1:9/stolen'); self.end_headers(); return
  if self.path.endswith('/rights'):
   data={'rights':[{'kind':{'CanReadAs':{'value':{'party':'A'}}}}]}
  else:
   data={'user':{'id':'source-reader','isDeactivated':False,'identityProviderId':''}}
  body=json.dumps(data).encode(); self.send_response(200); self.send_header('Content-Length',str(len(body))); self.end_headers(); self.wfile.write(body)
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain('server.pem','server.key')
server.socket=context.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True);server.serve_forever()
"#;
        let mut child = Command::new("python3")
            .args(["-c", script])
            .current_dir(&directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("TLS tests require python3");
        let mut port = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        let certificate = std::fs::read(directory.join("ca.pem")).unwrap();
        Self {
            directory,
            child,
            port: port.trim().parse().unwrap(),
            certificate,
        }
    }
    /// Add only the ephemeral test CA, preserving certificate/hostname verification and redirect refusal.
    fn checker(&self, host: &str, trusted: bool) -> GrantChecker {
        let mut checker = GrantChecker::configured(
            &format!("https://{host}:{}/", self.port),
            USER,
            scope(),
            [party()],
            TransportPolicy::RemoteHttps,
            "",
            None,
        )
        .unwrap();
        if trusted {
            checker.client = reqwest::Client::builder()
                .no_proxy()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .add_root_certificate(reqwest::Certificate::from_pem(&self.certificate).unwrap())
                .build()
                .unwrap();
        }
        checker
    }
}
/// Real handshakes prove that an unknown CA, wrong hostname and HTTPS downgrade never authorize a lease.
#[test]
fn verified_https_rejects_untrusted_certs_wrong_hosts_and_redirects() {
    let fixture = Fixture::start();
    assert!(matches!(
        runtime().block_on(fixture.checker("localhost", false).check(TOKEN)),
        Err(GrantError::Unavailable)
    ));
    assert!(
        runtime()
            .block_on(fixture.checker("localhost", true).check(TOKEN))
            .is_ok()
    );
    assert!(matches!(
        runtime().block_on(fixture.checker("127.0.0.1", true).check(TOKEN)),
        Err(GrantError::Unavailable)
    ));
    std::fs::write(fixture.directory.join("redirect"), b"test redirect").unwrap();
    assert!(matches!(
        runtime().block_on(fixture.checker("localhost", true).check(TOKEN)),
        Err(GrantError::Unavailable)
    ));
}
