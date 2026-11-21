//! Synthetic HTTP controls never contact a ledger or alter source permissions.
use super::*;
use cantongrid_core::visibility::CallerGrants;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

const USER: &str = "source-reader";
const ACTIVE: &str =
    r#"{"user":{"id":"source-reader","isDeactivated":false,"identityProviderId":""}}"#;
const RIGHTS: &str = r#"{"rights":[{"kind":{"CanReadAs":{"value":{"party":"A"}}}}]}"#;
// Deliberately unsigned synthetic data. Success here requires the fake server; a real
// Ledger rejects it. No local JWT decode result is accepted as a verified credential.
const TOKEN: &str =
    "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzb3VyY2UtcmVhZGVyIiwiZXhwIjo0MTAyNDQ0ODAwfQ.synthetic";

fn scope() -> ScopeBinding {
    ScopeBinding::new("synthetic-localnet", 1).unwrap()
}
fn party() -> PartyId {
    PartyId::new("A").unwrap()
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Serve a finite sequence, enforce GET path order and cap request input. The bearer
/// is checked for presence only and is never included in assertion diagnostics.
fn server(replies: Vec<(u16, String)>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let package_run = replies.len() == 7 || replies.len() == 4;
    let handle = thread::spawn(move || {
        for (index, (status, body)) in replies.into_iter().enumerate() {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut input = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0 && input.len() < 16384);
                input.extend_from_slice(&chunk[..n]);
                if input.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8(input).unwrap();
            let path = if package_run && index == 3 {
                "/v2/packages"
            } else if index == 1 || (package_run && index == 5) {
                "/v2/users/source-reader/rights"
            } else {
                "/v2/users/source-reader"
            };
            assert!(text.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
            assert!(text.to_ascii_lowercase().contains("authorization: bearer "));
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (endpoint, handle)
}

fn check(replies: Vec<(u16, String)>) -> Result<SourceLease, GrantError> {
    let (endpoint, server) = server(replies);
    let checker = GrantChecker::localnet(&endpoint, USER, scope(), [party()]).unwrap();
    let result = runtime().block_on(checker.check(TOKEN));
    server.join().unwrap();
    result
}

#[test]
fn verified_exact_read_grants_issue_a_scoped_expiring_lease() {
    let before = Instant::now();
    let lease = check(vec![
        (200, ACTIVE.into()),
        (200, RIGHTS.into()),
        (200, ACTIVE.into()),
    ])
    .unwrap();
    let caller = CallerGrants::from_verified(scope(), [party()]);
    assert_eq!(
        lease.read_parties(&scope(), &caller, Instant::now()),
        Some(vec![party()])
    );
    assert!(
        lease
            .read_parties(&scope(), &caller, before + MAX_SOURCE_LEASE + CHECK_TIMEOUT)
            .is_none()
    );
    assert!(
        lease
            .read_parties(
                &ScopeBinding::new("synthetic-localnet", 2).unwrap(),
                &caller,
                Instant::now()
            )
            .is_none()
    );
}

#[test]
fn unavailable_unauthorized_and_oversized_responses_never_issue_a_lease() {
    for (status, body, expected) in [
        (401, "private rejection".into(), GrantError::Denied),
        (403, "private rejection".into(), GrantError::Denied),
        (404, "missing user".into(), GrantError::Denied),
        (503, "private outage".into(), GrantError::Unavailable),
        (302, "redirect".into(), GrantError::Unavailable),
        (200, "not JSON".into(), GrantError::Response),
        (200, "x".repeat(MAX_BODY + 1), GrantError::Response),
    ] {
        assert_eq!(check(vec![(status, body)]).unwrap_err(), expected);
    }
}

#[test]
fn removed_widened_duplicate_and_privileged_grants_fail_closed() {
    for body in [
        r#"{"rights":[]}"#,
        r#"{"rights":[{"kind":{"CanReadAs":{"value":{"party":"B"}}}}]}"#,
        r#"{"rights":[{"kind":{"CanActAs":{"value":{"party":"A"}}}}]}"#,
        r#"{"rights":[{"kind":{"ParticipantAdmin":{"value":{}}}}]}"#,
        r#"{"rights":[{"kind":{"CanReadAsAnyParty":{"value":{}}}}]}"#,
        r#"{"rights":[{"kind":{"CanReadAs":{"value":{"party":"A"}}}},{"kind":{"CanReadAs":{"value":{"party":"A"}}}}]}"#,
        r#"{"rights":[{"kind":{"CanReadAs":{"value":{"party":"A"}},"CanActAs":{"value":{"party":"B"}}}}]}"#,
    ] {
        assert!(check(vec![(200, ACTIVE.into()), (200, body.into())]).is_err());
    }
}

#[test]
fn deactivated_replaced_and_other_provider_users_fail_closed() {
    for user in [
        r#"{"id":"source-reader","isDeactivated":true}"#,
        r#"{"id":"different-user"}"#,
        r#"{"id":"source-reader","identityProviderId":"other"}"#,
    ] {
        let body = format!(r#"{{"user":{user}}}"#);
        assert_eq!(
            check(vec![(200, body.clone())]).unwrap_err(),
            GrantError::ScopeChanged
        );
        assert_eq!(
            check(vec![
                (200, ACTIVE.into()),
                (200, RIGHTS.into()),
                (200, body)
            ])
            .unwrap_err(),
            GrantError::ScopeChanged
        );
    }
}

#[test]
fn invalid_tokens_and_endpoints_are_rejected_without_network_access() {
    for endpoint in [
        "https://127.0.0.1:3975",
        "http://localhost:3975",
        "http://192.0.2.1:3975",
        "http://user:secret@127.0.0.1:3975",
        "http://127.0.0.1:3975/path",
        "http://127.0.0.1:3975?x=y",
    ] {
        assert!(GrantChecker::localnet(endpoint, USER, scope(), [party()]).is_err());
    }
    let checker = GrantChecker::localnet("http://127.0.0.1:1", USER, scope(), [party()]).unwrap();
    for token in [
        "",
        "invalid",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzb3VyY2UtcmVhZGVyIiwiZXhwIjoxfQ.synthetic",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJvdGhlciIsImV4cCI6NDEwMjQ0NDgwMH0.synthetic",
    ] {
        assert_eq!(
            runtime().block_on(checker.check(token)).unwrap_err(),
            GrantError::Credential
        );
    }
    assert_eq!(
        runtime().block_on(checker.check_env("HOME")).unwrap_err(),
        GrantError::Configuration
    );
}

#[test]
fn failed_renewal_discards_previously_verified_authorization() {
    let (endpoint, server) = server(vec![
        (200, ACTIVE.into()),
        (200, RIGHTS.into()),
        (200, ACTIVE.into()),
    ]);
    let checker = GrantChecker::localnet(&endpoint, USER, scope(), [party()]).unwrap();
    let lease = runtime().block_on(checker.check(TOKEN)).unwrap();
    server.join().unwrap();
    let mut state = SourceAuthorization {
        checker,
        lease: Some(lease),
    };
    assert!(state.lease().is_some());
    assert_eq!(
        runtime().block_on(state.refresh_env("HOME")),
        Err(GrantError::Configuration)
    );
    assert!(state.lease().is_none());
}

#[test]
fn strict_configuration_rejects_secret_values_unknown_fields_and_duplicate_parties() {
    let valid = serde_json::json!({
        "endpoint":"http://127.0.0.1:3975", "user":"source-reader",
        "source":"local-provider", "network":"localnet", "generation":1,
        "parties":["A"], "credential_env":"CANTONGRID_SOURCE_TOKEN"
    });
    assert!(config::GrantCheckConfig::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for (key, value) in [
        ("token", serde_json::json!("never accept inline secrets")),
        ("credential_env", serde_json::json!("HOME")),
        ("credential_env", serde_json::json!("CANTONGRID_")),
        ("parties", serde_json::json!(["A", "A"])),
        ("parties", serde_json::json!([])),
        ("generation", serde_json::json!(0)),
    ] {
        let mut invalid = valid.clone();
        invalid[key] = value;
        assert!(config::GrantCheckConfig::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}

/// Package availability is authenticated read-only evidence; missing packages never become deployment proof.
#[test]
fn upgrade_packages_must_exist_on_the_authorized_participant() {
    for present in [true, false] {
        let id = "a".repeat(64);
        let packages = if present {
            serde_json::json!({"packageIds":[id]})
        } else {
            serde_json::json!({"packageIds":[]})
        };
        let mut replies = vec![
            (200, ACTIVE.into()),
            (200, RIGHTS.into()),
            (200, ACTIVE.into()),
            (200, packages.to_string()),
        ];
        if present {
            replies.extend([
                (200, ACTIVE.into()),
                (200, RIGHTS.into()),
                (200, ACTIVE.into()),
            ]);
        }
        let (endpoint, server) = server(replies);
        let checker = GrantChecker::localnet(&endpoint, USER, scope(), [party()]).unwrap();
        assert_eq!(
            runtime()
                .block_on(checker.require_packages(TOKEN, &[id]))
                .is_ok(),
            present
        );
        server.join().unwrap();
    }
}
