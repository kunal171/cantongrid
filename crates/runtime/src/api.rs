//! REST and GraphQL share one authorized SQL query path; neither transport supplies party grants.
use crate::{auth, config::Project, index::ready};
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::{PageKind, Store};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
struct Service {
    path: PathBuf,
    permits: Arc<tokio::sync::Semaphore>,
}
#[derive(Clone)]
struct RequestContext {
    service: Service,
    token: String,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Filter {
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Generic canonical records, an authorized total and a cursor scoped to the requesting principal.
#[derive(Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
struct Page {
    items: Vec<async_graphql::Json<serde_json::Value>>,
    visible_total: String,
    next_cursor: Option<String>,
    provenance: async_graphql::Json<serde_json::Value>,
}

impl Service {
    /// Revalidate caller and source before the blocking read, then revalidate caller again
    /// afterward. Source lease checks occur inside Store before/after SQL as well.
    async fn query(&self, token: &str, kind: PageKind, filter: Filter) -> Result<Page, String> {
        let project = Project::load(&self.path)?;
        let identity = auth::authenticate(&project, token)?;
        let limit = filter.limit.unwrap_or(20);
        if !(1..=25).contains(&limit) {
            return Err("invalid page limit".into());
        }
        let name = match kind {
            PageKind::Active => "active",
            PageKind::History => "history",
        };
        let after = filter
            .cursor
            .as_deref()
            .map(|c| auth::after(&project, &identity, name, c))
            .transpose()?;
        let mut authorization = SourceAuthorization::new(
            project
                .authorization
                .checker()
                .map_err(|_| "source authorization unavailable")?,
        );
        authorization
            .refresh_env(project.authorization.credential_env())
            .await
            .map_err(|_| "source authorization unavailable")?;
        ready(&project).await?;
        let caller = identity.caller.clone();
        let binding = identity.binding.clone();
        let path = self.path.clone();
        let mut page = tokio::task::spawn_blocking(move || {
            let lease = authorization
                .lease()
                .ok_or("source authorization unavailable")?;
            let mut store = Store::open_env(&project.destination_database_env)
                .map_err(|_| "destination unavailable")?;
            let page = store
                .read_page(&project.plan, &caller, lease, kind, after.as_deref(), limit)
                .map_err(|e| format!("authorized query unavailable: {e}"))?;
            Ok::<_, String>(page)
        })
        .await
        .map_err(|_| "query worker failed")??;
        let project = Project::load(&path)?;
        let _source = current_source(&project).await?;
        let current = auth::authenticate(&project, token)?;
        if current.binding != binding {
            return Err("authorization changed".into());
        }
        let next_cursor = page
            .next_after
            .take()
            .map(|key| auth::cursor(&project, &current, name, key))
            .transpose()?;
        let cursor = page
            .checkpoint
            .as_deref()
            .and_then(|text| serde_json::from_str::<cantongrid_pqs::Cursor>(text).ok());
        let (source, network, generation) = project.plan.context().identity();
        let coverage = match &cursor {
            Some(c) if c.snapshot => "bootstrapping",
            Some(_) => "partial_history",
            None => "unknown",
        };
        let result = Page {
            items: page
                .items
                .into_iter()
                .map(|item| {
                    if matches!(kind, PageKind::History) {
                        public_event(item)
                    } else {
                        item
                    }
                })
                .map(async_graphql::Json)
                .collect(),
            visible_total: page.visible_total.to_string(),
            next_cursor,
            provenance: async_graphql::Json(serde_json::json!({
                "source":source,"network":network,"generation":generation,"coverage":coverage,
                "sourceLocalWatermark":cursor.as_ref().map(|c|c.boundary.to_string()),
            "historyFromOffset":page.first_checkpoint.as_deref().and_then(|text|serde_json::from_str::<cantongrid_pqs::Cursor>(text).ok()).map(|c|c.boundary.to_string()),
                "observedAgeSeconds":page.observed_age_seconds,
                "freshness":if page.observed_age_seconds.is_some_and(|s|s<=30){"recently_observed"}else{"stale_or_unknown"},
                "payloadEncoding":"raw_json","archiveEvidence":"prior_create_dependency"
            })),
        };
        if serde_json::to_vec(&result)
            .map_err(|_| "query encoding failed")?
            .len()
            > 2 * 1024 * 1024
        {
            return Err("query response exceeds limit".into());
        }
        if !source_valid(&_source, &project)
            || auth::authenticate(&project, token)?.binding != binding
        {
            return Err("authorization expired or changed before delivery".into());
        }
        Ok(result)
    }
}

/// Source witness sets and internal page PKs belong to the operator, not narrower API
/// principals. Expose record-local provenance without the raw persistence envelope.
fn public_event(item: serde_json::Value) -> serde_json::Value {
    let event = &item["envelope"]["events"][0];
    serde_json::json!({
        "eventId":item["eventId"],"contractId":item["contractId"],"kind":item["kind"],
        "template":event["template"],"payload":event["action"].get("payload"),
        "observedAtMicros":event["observed_at_micros"].as_i64().map(|v|v.to_string()),
        "ledgerAtMicros":event["ledger_at_micros"].as_i64().map(|v|v.to_string()),
        "sourceOffset":event["origin"]["offset"].as_i64().map(|v|v.to_string()),
        "snapshotObservation":event["origin"]["snapshot_observation"],
        "visibilityEvidence":if event["action"]["kind"]=="archive_by_prior_contract" {"prior_create_policy"}else{"source_witness_policy"}
    })
}

struct QueryRoot;
#[Object]
impl QueryRoot {
    #[graphql(complexity = 50)]
    async fn active_contracts(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> async_graphql::Result<Page> {
        let request = ctx.data_unchecked::<RequestContext>();
        request
            .service
            .query(
                &request.token,
                PageKind::Active,
                Filter {
                    limit: limit.map(|n| n as usize),
                    cursor,
                },
            )
            .await
            .map_err(|reason| {
                eprintln!("CantonGrid query: {reason}");
                async_graphql::Error::new("authorized query unavailable")
            })
    }
    #[graphql(complexity = 50)]
    async fn lifecycle_events(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> async_graphql::Result<Page> {
        let request = ctx.data_unchecked::<RequestContext>();
        request
            .service
            .query(
                &request.token,
                PageKind::History,
                Filter {
                    limit: limit.map(|n| n as usize),
                    cursor,
                },
            )
            .await
            .map_err(|reason| {
                eprintln!("CantonGrid query: {reason}");
                async_graphql::Error::new("authorized query unavailable")
            })
    }
}

/// Refresh current source authorization before final transport delivery, including multi-root
/// GraphQL operations whose earlier resolver lease may have expired while a later root ran.
async fn current_source(project: &Project) -> Result<SourceAuthorization, String> {
    let mut source = SourceAuthorization::new(
        project
            .authorization
            .checker()
            .map_err(|_| "source authorization unavailable")?,
    );
    source
        .refresh_env(project.authorization.credential_env())
        .await
        .map_err(|_| "source authorization unavailable")?;
    ready(project).await?;
    if !source_valid(&source, project) {
        return Err("source authorization expired".into());
    }
    Ok(source)
}

/// Health checks and serialization also consume lease time; check after those awaits/work.
fn source_valid(source: &SourceAuthorization, project: &Project) -> bool {
    source
        .lease()
        .and_then(|lease| {
            lease.source_parties(&project.plan.context().binding(), std::time::Instant::now())
        })
        .is_some()
}

/// Accept exactly one bounded bearer; avoid ambiguity between multiple Authorization headers.
fn bearer(headers: &HeaderMap) -> Result<String, StatusCode> {
    if headers.get_all("authorization").iter().count() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let value = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|s| !s.is_empty() && s.len() <= 8192)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(value.into())
}
fn error(status: StatusCode) -> Response {
    (
        status,
        Json(serde_json::json!({"error":"request unavailable"})),
    )
        .into_response()
}

async fn rest(service: Service, headers: HeaderMap, filter: Filter, kind: PageKind) -> Response {
    let Ok(_permit) = service.permits.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
    let token = match bearer(&headers) {
        Ok(t) => t,
        Err(r) => return error(r),
    };
    let Ok(project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    if auth::authenticate(&project, &token).is_err() {
        return error(StatusCode::UNAUTHORIZED);
    }
    match service.query(&token, kind, filter).await {
        Ok(page) => Json(page).into_response(),
        Err(reason) => {
            eprintln!("CantonGrid query: {reason}");
            error(
                if reason == "invalid page limit" || reason == "invalid cursor" {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
            )
        }
    }
}
async fn active(
    State(service): State<Service>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    rest(service, headers, filter, PageKind::Active).await
}
async fn history(
    State(service): State<Service>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    rest(service, headers, filter, PageKind::History).await
}

/// Check caller again after the complete GraphQL operation, so a mid-operation grant change
/// cannot expose a successful earlier resolver alongside later denial errors.
async fn graphql(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(request): Json<async_graphql::Request>,
) -> Response {
    let Ok(_permit) = service.permits.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS);
    };
    let token = match bearer(&headers) {
        Ok(t) => t,
        Err(r) => return error(r),
    };
    let Ok(project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(identity) = auth::authenticate(&project, &token) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .limit_depth(8)
        .limit_complexity(100)
        .disable_introspection()
        .finish();
    let response = schema
        .execute(request.data(RequestContext {
            service: service.clone(),
            token: token.clone(),
        }))
        .await;
    let Ok(latest_project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(_source) = current_source(&latest_project).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let current = auth::authenticate(&latest_project, &token);
    if !current.is_ok_and(|id| id.binding == identity.binding) {
        return error(StatusCode::UNAUTHORIZED);
    }
    let response = Json(response).into_response();
    if !source_valid(&_source, &latest_project)
        || !auth::authenticate(&latest_project, &token)
            .is_ok_and(|id| id.binding == identity.binding)
    {
        return error(StatusCode::UNAUTHORIZED);
    }
    response
}

/// Bind only configured loopback addresses, cap request bodies and bound concurrent DB work.
/// No playground, mutation, subscription, arbitrary SQL or unauthenticated data route is mounted.
pub async fn serve(path: &Path) -> Result<(), String> {
    let project = Project::load(path)?;
    let listener = tokio::net::TcpListener::bind(project.api.bind)
        .await
        .map_err(|_| "cannot bind API listener")?;
    let service = Service {
        path: path.to_owned(),
        permits: Arc::new(tokio::sync::Semaphore::new(16)),
    };
    let router = Router::new()
        .route("/v1/contracts", get(active))
        .route("/v1/events", get(history))
        .route("/graphql", post(graphql))
        .layer(axum::middleware::from_fn(no_store))
        .layer(DefaultBodyLimit::max(16384))
        .with_state(service);
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "API server failed".into())
}

/// Private contract responses are never eligible for browser or intermediary caching.
async fn no_store(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
