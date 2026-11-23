//! Bootstrap transport never accepts or returns the private source boundary used by storage.
use super::*;

/// Caller-controlled names only; source identity, scope and anchor are derived by the server.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::api) struct BootstrapRegistration {
    name: String,
}
/// Snapshot cursor domains are private to one subscription and separate from its delivery receipts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::api) struct SnapshotFilter {
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Commit a fixed baseline and feed anchor together, under current source and organization authority.
pub(in crate::api) async fn bootstrap(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(body): Json<BootstrapRegistration>,
) -> Response {
    rest::execute(service, headers, Operation::Bootstrap { name: body.name }).await
}

/// Page only the baseline already bound to this subscription; clients cannot select another offset.
pub(in crate::api) async fn snapshot(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Query(filter): Query<SnapshotFilter>,
) -> Response {
    let limit = filter.limit.unwrap_or(20);
    if !(1..=25).contains(&limit) {
        return error(StatusCode::BAD_REQUEST);
    }
    rest::execute(
        service,
        headers,
        Operation::Snapshot {
            id,
            cursor: filter.cursor,
            limit,
        },
    )
    .await
}
