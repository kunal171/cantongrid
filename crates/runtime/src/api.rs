//! REST and GraphQL share one authorized SQL query path; neither transport supplies party grants.
pub(crate) mod delivery;
mod health;
mod model_api;
mod resources;
mod temporal;

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
use cantongrid_storage::{LineageDirection, LineageQuery, PageKind, Store};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Shared configuration location and resource budgets; each request reloads policy instead of caching grants.
#[derive(Clone)]
struct Service {
    path: PathBuf,
    limits: resources::Limits,
}
/// One verified request bearer shared by its resolvers; fields cannot supply their own party grants.
#[derive(Clone)]
struct RequestContext {
    service: Service,
    token: String,
}
/// Generic page arguments only; caller identity and visibility never come from query parameters.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Filter {
    limit: Option<usize>,
    cursor: Option<String>,
}

/// Trusted query dispatch; root/direction are data inputs, never grant claims.
enum Selection {
    Canonical(PageKind),
    Temporal {
        offset: i64,
    },
    Transaction {
        root: String,
    },
    Lineage {
        root: String,
        direction: LineageDirection,
    },
}
impl Selection {
    /// Cursor domains prevent cross-root, cross-direction and canonical/lineage replay.
    fn cursor_kind(&self) -> String {
        match self {
            Self::Canonical(PageKind::Active) => "active".into(),
            Self::Canonical(PageKind::History) => "history".into(),
            Self::Temporal { offset } => format!("as-of:{offset}"),
            Self::Transaction { root } => {
                use sha2::{Digest, Sha256};
                format!(
                    "inspect-transaction:{}",
                    Sha256::digest(root.as_bytes())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                )
            }
            Self::Lineage { root, direction } => {
                use sha2::{Digest, Sha256};
                let hash =
                    Sha256::digest(serde_json::json!([root, direction.as_str()]).to_string());
                format!(
                    "lineage:{}",
                    hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
                )
            }
        }
    }
}

/// REST exposes only one-hop directions. Unknown filter fields remain rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageFilter {
    direction: LineageDirection,
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

/// Keep the final verified source lease alive through REST encoding. GraphQL resolvers extract
/// only the page and use a fresh operation-wide guard after all roots have completed.
struct AuthorizedPage {
    page: Page,
    project: Project,
    source: SourceAuthorization,
    binding: String,
}
impl AuthorizedPage {
    /// Recheck both authorities after the bounded body has been fully encoded.
    fn response(&self, token: &str) -> Response {
        resources::guarded_json(&self.page, || {
            source_valid(&self.source, &self.project)
                && auth::authenticate(&self.project, token)
                    .is_ok_and(|id| id.binding == self.binding)
        })
    }
}

impl Service {
    /// Revalidate caller and source before the blocking read, then revalidate caller again
    /// afterward. Source lease checks occur inside Store before/after SQL as well.
    async fn query(
        &self,
        token: &str,
        kind: PageKind,
        filter: Filter,
    ) -> Result<AuthorizedPage, String> {
        self.query_selection(token, Selection::Canonical(kind), filter)
            .await
    }

    /// Reuse the canonical authority/encoding guards for direct lineage as well as lifecycle pages.
    async fn query_selection(
        &self,
        token: &str,
        selection: Selection,
        filter: Filter,
    ) -> Result<AuthorizedPage, String> {
        if let Selection::Lineage { root, .. } = &selection
            && (root.is_empty()
                || root.len() > 1024
                || root.trim() != root
                || root.chars().any(char::is_control))
        {
            return Err("invalid lineage filter".into());
        }
        if let Selection::Transaction { root } = &selection
            && (root.is_empty()
                || root.len() > 1024
                || root.trim() != root
                || root.chars().any(char::is_control))
        {
            return Err("invalid inspector filter".into());
        }
        let project = Project::load(&self.path)?;
        let identity = auth::authenticate(&project, token)?;
        let limit = filter.limit.unwrap_or(20);
        if !(1..=25).contains(&limit) {
            return Err("invalid page limit".into());
        }
        let historical_offset = match &selection {
            Selection::Temporal { offset } => Some(*offset),
            _ => None,
        };
        let name = selection.cursor_kind();
        let history = matches!(selection, Selection::Canonical(PageKind::History));
        let inspecting = matches!(selection, Selection::Transaction { .. });
        let lineage = matches!(selection, Selection::Lineage { .. });
        let after = filter
            .cursor
            .as_deref()
            .map(|c| auth::after(&project, &identity, &name, c))
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
        let mut page = self
            .limits
            .blocking(move || {
                let lease = authorization
                    .lease()
                    .ok_or("source authorization unavailable")?;
                let mut store = Store::open_env(&project.destination_database_env)
                    .map_err(|_| "destination unavailable")?;
                let page = match selection {
                    Selection::Transaction { root } => store.inspect_transaction(
                        &project.plan,
                        &caller,
                        lease,
                        &root,
                        after.as_deref(),
                        limit,
                    ),
                    Selection::Temporal { offset } => store.read_as_of(
                        &project.plan,
                        &caller,
                        lease,
                        cantongrid_storage::TemporalQuery {
                            offset,
                            after: after.as_deref(),
                            limit,
                        },
                    ),
                    Selection::Canonical(kind) => store.read_page(
                        &project.plan,
                        &caller,
                        lease,
                        kind,
                        after.as_deref(),
                        limit,
                    ),
                    Selection::Lineage { root, direction } => store.read_lineage(
                        &project.plan,
                        &caller,
                        lease,
                        LineageQuery {
                            root: &root,
                            direction,
                            after: after.as_deref(),
                            limit,
                        },
                    ),
                }
                .map_err(|e| {
                    if e == cantongrid_storage::StoreError::TemporalUnavailable {
                        "temporal coverage unavailable".into()
                    } else if e == cantongrid_storage::StoreError::LineageUnavailable
                        || e == cantongrid_storage::StoreError::InspectorUnavailable
                    {
                        "contract unavailable".into()
                    } else if e == cantongrid_storage::StoreError::ModelQuery {
                        "invalid inspector filter".into()
                    } else {
                        format!("authorized query unavailable: {e}")
                    }
                })?;
                Ok::<_, String>(page)
            })
            .await
            .map_err(|_| "query worker failed")??;
        let project = Project::load(&path)?;
        let source_authorization = current_source(&project).await?;
        let current = auth::authenticate(&project, token)?;
        if current.binding != binding {
            return Err("authorization changed".into());
        }
        let next_cursor = page
            .next_after
            .take()
            .map(|key| auth::cursor(&project, &current, &name, key))
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
        let mut result = Page {
            items: page
                .items
                .into_iter()
                .map(|item| if history { public_event(item) } else { item })
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
        if let Some(offset) = historical_offset {
            result.provenance = async_graphql::Json(serde_json::json!({
                "source":source,"network":network,"generation":generation,
                "coverage":"observed_retained","completeWithinConfiguredScope":true,
                "asOfOffset":offset.to_string(),"historyFromOffset":result.provenance.0["historyFromOffset"],
                "timeDomain":"source_local_offset","projection":"canonical_immutable_contracts",
                "modelRevision":null,"applicationEffectiveTime":"not_inferred"
            }));
        }
        if inspecting {
            result.provenance = async_graphql::Json(
                serde_json::json!({"coverage":"partial_recorded_evidence","complete":false,"view":"flat_authorized_records","order":"visible_record_ordinal","payloadsIncluded":false}),
            );
        }
        if lineage {
            result.provenance.0["coverage"] = serde_json::json!("partial_recorded_evidence");
            result.provenance.0["complete"] = serde_json::json!(false);
            result.provenance.0["depth"] = serde_json::json!(1);
        }
        if !source_valid(&source_authorization, &project)
            || auth::authenticate(&project, token)?.binding != binding
        {
            return Err("authorization expired or changed before delivery".into());
        }
        Ok(AuthorizedPage {
            page: result,
            project,
            source: source_authorization,
            binding,
        })
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

/// Read-only generic contract/history roots; generated model queries have a separate schema.
struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Canonical state at a proven boundary, always under current caller and source authority.
    #[graphql(complexity = 50)]
    async fn contracts_as_of(
        &self,
        ctx: &Context<'_>,
        as_of_offset: String,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> async_graphql::Result<Page> {
        use async_graphql::ErrorExtensions;
        let offset = temporal::offset(&as_of_offset).map_err(async_graphql::Error::new)?;
        let request = ctx.data_unchecked::<RequestContext>();
        request
            .service
            .query_selection(
                &request.token,
                Selection::Temporal { offset },
                Filter {
                    limit: limit.map(|v| v as usize),
                    cursor,
                },
            )
            .await
            .map(|r| r.page)
            .map_err(|reason| {
                let error = async_graphql::Error::new("authorized temporal query unavailable");
                if reason == "temporal coverage unavailable" {
                    error.extend_with(|_, e| e.set("coverage", "unavailable"))
                } else {
                    error
                }
            })
    }

    /// Flat authorized records; neither a transaction ID nor an exercise observer grants endpoint access.
    #[graphql(complexity = 50)]
    async fn inspect_transaction(
        &self,
        ctx: &Context<'_>,
        transaction_id: String,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> async_graphql::Result<Page> {
        let request = ctx.data_unchecked::<RequestContext>();
        request
            .service
            .query_selection(
                &request.token,
                Selection::Transaction {
                    root: transaction_id,
                },
                Filter {
                    limit: limit.map(|v| v as usize),
                    cursor,
                },
            )
            .await
            .map(|r| r.page)
            .map_err(|_| async_graphql::Error::new("authorized query unavailable"))
    }

    /// A visible root does not imply visibility of its relationships or proof participants.
    #[graphql(complexity = 50)]
    async fn contract_lineage(
        &self,
        ctx: &Context<'_>,
        contract_id: String,
        direction: String,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> async_graphql::Result<Page> {
        let direction: LineageDirection =
            serde_json::from_value(serde_json::Value::String(direction))
                .map_err(|_| async_graphql::Error::new("invalid lineage direction"))?;
        let request = ctx.data_unchecked::<RequestContext>();
        request
            .service
            .query_selection(
                &request.token,
                Selection::Lineage {
                    root: contract_id,
                    direction,
                },
                Filter {
                    limit: limit.map(|v| v as usize),
                    cursor,
                },
            )
            .await
            .map(|result| result.page)
            .map_err(|_| async_graphql::Error::new("authorized query unavailable"))
    }
    /// Resolve a caller-visible current-state page using the same authorized service path as REST.
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
            .map(|result| result.page)
            .map_err(|reason| {
                eprintln!("CantonGrid query: {reason}");
                async_graphql::Error::new("authorized query unavailable")
            })
    }
    /// Resolve lifecycle history with its own cursor kind and inherited visibility evidence.
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
            .map(|result| result.page)
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
/// Return one bounded public failure shape without source, SQL or private payload diagnostics.
fn error(status: StatusCode) -> Response {
    (
        status,
        Json(serde_json::json!({"error":"request unavailable"})),
    )
        .into_response()
}

/// Authenticate a generic REST read and retain its final source lease through response encoding.
async fn rest(
    service: Service,
    headers: HeaderMap,
    filter: Filter,
    selection: Selection,
) -> Response {
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
    match service.query_selection(&token, selection, filter).await {
        Ok(page) => page.response(&token),
        Err(reason) => {
            eprintln!("CantonGrid query: {reason}");
            if reason == "temporal coverage unavailable" {
                return (StatusCode::CONFLICT,Json(serde_json::json!({"error":"temporal state unavailable","coverage":"unavailable"}))).into_response();
            }
            error(if reason == "contract unavailable" {
                StatusCode::NOT_FOUND
            } else if reason == "invalid page limit"
                || reason == "invalid cursor"
                || reason == "invalid lineage filter"
                || reason == "invalid inspector filter"
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            })
        }
    }
}
/// Expose current contracts through the common REST guard and strict page extractor.
async fn active(
    State(service): State<Service>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    rest(
        service,
        headers,
        filter,
        Selection::Canonical(PageKind::Active),
    )
    .await
}
/// Expose history through the common REST guard, keeping its cursor distinct from active rows.
async fn history(
    State(service): State<Service>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    rest(
        service,
        headers,
        filter,
        Selection::Canonical(PageKind::History),
    )
    .await
}

/// Root visibility uses the same opaque unavailable response for unknown and unauthorized IDs.
async fn lineage(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(root): axum::extract::Path<String>,
    Query(filter): Query<LineageFilter>,
) -> Response {
    rest(
        service,
        headers,
        Filter {
            limit: filter.limit,
            cursor: filter.cursor,
        },
        Selection::Lineage {
            root,
            direction: filter.direction,
        },
    )
    .await
}

/// Inspector pagination uses visible ordinals, never source node positions.
async fn transaction(
    State(service): State<Service>,
    headers: HeaderMap,
    axum::extract::Path(root): axum::extract::Path<String>,
    Query(filter): Query<Filter>,
) -> Response {
    rest(service, headers, filter, Selection::Transaction { root }).await
}

/// Check caller again after the complete GraphQL operation, so a mid-operation grant change
/// cannot expose a successful earlier resolver alongside later denial errors.
async fn graphql(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(request): Json<async_graphql::Request>,
) -> Response {
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
    resources::guarded_json(&response, || {
        source_valid(&_source, &latest_project)
            && auth::authenticate(&latest_project, &token)
                .is_ok_and(|id| id.binding == identity.binding)
    })
}

/// Bind only configured loopback addresses, cap request bodies and bound concurrent DB work.
/// Durable subscription writes affect owned consumer metadata only; no ledger mutation is exposed.
pub async fn serve(path: &Path) -> Result<(), String> {
    let project = Project::load(path)?;
    let listener = tokio::net::TcpListener::bind(project.api.bind)
        .await
        .map_err(|_| "cannot bind API listener")?;
    let service = Service {
        path: path.to_owned(),
        limits: resources::Limits::default(),
    };
    let router = Router::new()
        .route("/v1/health", get(health::readiness))
        .route("/v1/subscriptions", post(delivery::subscribe))
        .route("/v1/subscriptions/bootstrap", post(delivery::bootstrap))
        .route(
            "/v1/subscriptions/{subscriptionId}/snapshot",
            get(delivery::snapshot),
        )
        .route(
            "/v1/subscriptions/{subscriptionId}/events",
            get(delivery::events),
        )
        .route(
            "/v1/subscriptions/{subscriptionId}/acknowledge",
            post(delivery::acknowledge),
        )
        .route(
            "/v1/subscriptions/{subscriptionId}/cancel",
            post(delivery::cancel),
        )
        .route("/v1/contracts", get(active))
        .route("/v1/contracts/as-of", get(temporal::state))
        .route("/v1/inspect/transactions/{transactionId}", get(transaction))
        .route("/v1/contracts/{contractId}/lineage", get(lineage))
        .route("/v1/events", get(history))
        .route("/graphql", post(graphql))
        .route("/models/graphql", post(model_api::graphql))
        .route("/v1/models", get(model_api::list))
        .route("/v1/models/{namespace}/{name}", get(model_api::describe))
        .route("/v1/models/{namespace}/{name}/query", post(model_api::rows))
        .route(
            "/v1/models/{namespace}/{name}/inspect",
            post(model_api::diagnose),
        )
        .route(
            "/v1/models/{namespace}/{name}/joins/{relationship}/query",
            post(model_api::join_rows),
        )
        .route(
            "/v1/models/{namespace}/{name}/relationships/{relationship}/query",
            post(model_api::relationship_rows),
        )
        .route(
            "/v1/models/{namespace}/{name}/aggregate",
            post(model_api::aggregates),
        )
        .layer(DefaultBodyLimit::max(16384))
        .layer(axum::middleware::from_fn_with_state(
            service.limits.clone(),
            resources::admit,
        ))
        .layer(axum::middleware::from_fn(no_store))
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

/// Operator status publishes fixed budgets only, never current global request activity.
pub(crate) fn resource_limits() -> serde_json::Value {
    resources::document()
}
