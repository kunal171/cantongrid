//! Real transport and cancellation controls; no destination or source credentials are used.
use super::*;

#[test]
fn cancellation_does_not_release_a_running_database_slot() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let limits = Limits {
            database: Arc::new(Semaphore::new(1)),
            ..Limits::default()
        };
        let worker_limits = limits.clone();
        let (started, observed) = tokio::sync::oneshot::channel();
        let (finish, wait) = std::sync::mpsc::channel();
        let waiter = tokio::spawn(async move {
            worker_limits
                .blocking(move || {
                    started.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                })
                .await
        });
        observed.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(limits.blocking(|| ()).await.is_err());
        finish.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while limits.database.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(limits.blocking(|| ()).await.is_ok());
    });
}

#[test]
fn aggregate_response_limit_includes_json_escaping_and_all_resolvers() {
    let page = "a".repeat(1100 * 1024);
    assert_eq!(json(&page).status(), StatusCode::OK);
    assert_eq!(
        json(&serde_json::json!({"data":{"first":page,"second":page}})).status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        json(&"\0".repeat(MAX_RESPONSE / 6 + 1)).status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(json(&"a".repeat(MAX_RESPONSE - 2)).status(), StatusCode::OK);
}

#[test]
fn slow_request_bodies_are_admitted_before_extraction_and_expire() {
    use axum::{Router, body::Bytes, routing::post};
    use std::io::{Read, Write};
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let limits = Limits { requests: Arc::new(Semaphore::new(1)), deadline: Duration::from_secs(2), ..Limits::default() };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route("/", post(|_: Bytes| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(limits.clone(), admit));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
        // Send only one of ten promised bytes; the extractor cannot finish this request.
        let mut slow = std::net::TcpStream::connect(address).unwrap();
        slow.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        slow.write_all(b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\nConnection: close\r\n\r\nx").unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while limits.requests.available_permits() != 0 { tokio::task::yield_now().await; }
        }).await.unwrap();
        let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap();
        let url = format!("http://{address}/");
        assert_eq!(client.post(&url).body("complete").send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
        let response = tokio::task::spawn_blocking(move || {
            let mut response = String::new();
            slow.read_to_string(&mut response).unwrap();
            response
        }).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 504"));
        assert_eq!(client.post(&url).body("complete").send().await.unwrap().status(), StatusCode::OK);
        server.abort();
    });
}
