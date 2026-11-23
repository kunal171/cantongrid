//! Authenticated readiness exposes no party data, counters, cursor positions or source connection details.
use super::*;

/// Report only service readiness after current caller/source checks and a verified owned prefix.
pub(super) async fn readiness(State(service): State<Service>, headers: HeaderMap) -> Response {
    let token = match bearer(&headers) {
        Ok(token) => token,
        Err(status) => return error(status),
    };
    let Ok(project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(identity) = auth::authenticate(&project, &token) else {
        return error(StatusCode::UNAUTHORIZED);
    };
    let binding = identity.binding;
    let Ok(source) = current_source(&project).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let checked = service
        .limits
        .blocking(move || {
            let Some(lease) = source.lease() else {
                return false;
            };
            if lease
                .source_parties(&project.plan.context().binding(), std::time::Instant::now())
                .is_none()
            {
                return false;
            }
            Store::open_env(&project.destination_database_env)
                .and_then(|mut s| s.ready_for_live_delivery(&project.plan, lease))
                .unwrap_or(false)
        })
        .await;
    if !matches!(checked, Ok(true)) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Ok(project) = Project::load(&service.path) else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(source) = current_source(&project).await else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    resources::guarded_json(
        &serde_json::json!({"status":"ready","formatVersion":1}),
        || {
            source_valid(&source, &project)
                && auth::authenticate(&project, &token)
                    .is_ok_and(|identity| identity.binding == binding)
        },
    )
}
