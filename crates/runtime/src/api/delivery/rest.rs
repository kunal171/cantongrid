//! Authenticated registration, bounded long polling, acknowledgement and cancellation.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::api) struct Registration {
    name: String,
    start: DeliveryStart,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in crate::api) struct FeedFilter {
    cursor: Option<String>,
    limit: Option<usize>,
    wait_seconds: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::api) struct Acknowledgement {
    cursor: String,
}

/// Authenticate before creating any private consumer state; no ledger command is submitted.
pub(in crate::api) async fn subscribe(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(body): Json<Registration>,
) -> Response {
    execute(
        service,
        headers,
        Operation::Register {
            name: body.name,
            start: body.start,
        },
    )
    .await
}
/// Poll immediately or wait up to ten seconds; reauthorize every iteration and before encoding.
pub(in crate::api) async fn events(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Query(filter): Query<FeedFilter>,
) -> Response {
    let token = match bearer(&headers) {
        Ok(t) => t,
        Err(status) => return error(status),
    };
    let wait = filter.wait_seconds.unwrap_or(0);
    let limit = filter.limit.unwrap_or(20);
    if wait > 10 || !(1..=25).contains(&limit) {
        return error(StatusCode::BAD_REQUEST);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait);
    loop {
        match service
            .delivery(
                &token,
                Operation::Read {
                    id: id.clone(),
                    cursor: filter.cursor.clone(),
                    limit,
                },
            )
            .await
        {
            Ok(reply) => {
                if reply.data["events"]
                    .as_array()
                    .is_some_and(|v| v.is_empty())
                    && std::time::Instant::now() < deadline
                {
                    drop(reply);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                } else {
                    return reply.response(&token);
                }
            }
            Err(failure) => return failure.response(),
        }
    }
}
/// The caller acknowledges a processed contiguous prefix using a signed issued receipt.
pub(in crate::api) async fn acknowledge(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<Acknowledgement>,
) -> Response {
    execute(
        service,
        headers,
        Operation::Acknowledge {
            id,
            cursor: body.cursor,
        },
    )
    .await
}
/// Cancel only this caller's live subscription; unknown and wrong-owner IDs share one result.
pub(in crate::api) async fn cancel(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    execute(service, headers, Operation::Cancel { id }).await
}
/// Metadata writes and event reads share the same final authority/encoding guard.
async fn execute(service: Service, headers: HeaderMap, operation: Operation) -> Response {
    let token = match bearer(&headers) {
        Ok(t) => t,
        Err(status) => return error(status),
    };
    match service.delivery(&token, operation).await {
        Ok(reply) => reply.response(&token),
        Err(failure) => failure.response(),
    }
}
