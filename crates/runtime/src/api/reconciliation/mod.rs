//! Independent caller/source authorization around server-selected, immutable reconciliation policies.
mod graphql;
mod rest;
use super::{Service, auth, current_source, error, resources};
use crate::{
    config::Project,
    reconciliation::{Catalog, Entry},
};
use axum::{http::StatusCode, response::Response};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_storage::{BoundModel, Store};
pub(super) use graphql::handle as graphql;
pub(super) use rest::{list, query};
use serde::Serialize;
use std::time::Instant;

/// One admitted operation's private context; it is never serialized or constructed from an HTTP body.
struct Prepared {
    project: Project,
    identity: auth::Identity,
    source: SourceAuthorization,
    catalog: Catalog,
    bindings: Vec<BoundModel>,
}
/// Finite public transport classifications; raw driver/parser/source diagnostics remain private.
#[derive(Clone, Copy)]
enum Failure {
    Unauthorized,
    Forbidden,
    Input,
    Missing,
    Unavailable,
}
impl Failure {
    /// Permission failures do not imply a policy or model exists; only full-scope callers reach lookup.
    fn status(self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Input => StatusCode::BAD_REQUEST,
            Self::Missing => StatusCode::NOT_FOUND,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}
/// Exact source grants plus full caller scope, independently of current rows or configured policy names.
fn full_scope(
    project: &Project,
    identity: &auth::Identity,
    source: &SourceAuthorization,
) -> Result<(), Failure> {
    let scope = project.plan.context().binding();
    let lease = source.lease().ok_or(Failure::Unavailable)?;
    let expected = project.plan.source_parties().cloned().collect();
    if lease.source_parties(&scope, Instant::now()) != Some(&expected) {
        return Err(Failure::Unavailable);
    }
    let parties = lease
        .read_parties(&scope, &identity.caller, Instant::now())
        .ok_or(Failure::Forbidden)?;
    if parties.len() != project.plan.source_parties().count() {
        return Err(Failure::Forbidden);
    }
    Ok(())
}
/// Resolve an immutable operator pin against trusted discovery, never a caller-supplied model identity.
fn binding<'a>(prepared: &'a Prepared, entry: &Entry) -> Result<&'a BoundModel, Failure> {
    prepared
        .bindings
        .iter()
        .find(|binding| {
            binding.namespace == entry.pin.namespace
                && binding.model.name == entry.pin.model
                && binding.model.revision.to_string() == entry.pin.model_revision
                && binding.fingerprint == entry.pin.model_fingerprint
        })
        .ok_or(Failure::Unavailable)
}
/// Authenticate and require complete scope before loading policy files or querying metadata/counts.
async fn prepare(service: &Service, token: &str) -> Result<Prepared, Failure> {
    let project = Project::load(&service.path).map_err(|_| Failure::Unavailable)?;
    let identity = auth::authenticate(&project, token).map_err(|_| Failure::Unauthorized)?;
    let source = current_source(&project)
        .await
        .map_err(|_| Failure::Unavailable)?;
    full_scope(&project, &identity, &source)?;
    let catalog =
        Catalog::load(project.reconciliation_file.as_deref()).map_err(|_| Failure::Unavailable)?;
    service
        .limits
        .blocking(move || {
            let bindings = Store::open_env(&project.destination_database_env)
                .map_err(|_| Failure::Unavailable)?
                .bound_models(
                    &project.plan,
                    &identity.caller,
                    source.lease().ok_or(Failure::Unavailable)?,
                )
                .map_err(|_| Failure::Unavailable)?;
            let prepared = Prepared {
                project,
                identity,
                source,
                catalog,
                bindings,
            };
            // Fail the configured catalog together: do not advertise an inactive or incompatible pin as executable.
            for entry in &prepared.catalog.entries {
                let model = binding(&prepared, entry)?;
                entry
                    .policy
                    .preflight_model(&model.namespace, &model.model)
                    .map_err(|_| Failure::Unavailable)?;
            }
            full_scope(&prepared.project, &prepared.identity, &prepared.source)?;
            Ok(prepared)
        })
        .await
        .map_err(|_| Failure::Unavailable)?
}
/// Include all active identities and the exact source plan; definition hashes alone are not source capabilities.
fn same_context(a: &Prepared, b: &Prepared) -> bool {
    let identities = |p: &Prepared| {
        p.bindings
            .iter()
            .map(|m| (m.namespace.clone(), m.fingerprint.clone()))
            .collect::<Vec<_>>()
    };
    a.identity.binding == b.identity.binding
        && a.catalog.fingerprint == b.catalog.fingerprint
        && a.project.plan.definition() == b.project.plan.definition()
        && identities(a) == identities(b)
}
/// Execute only a configured alias under freshly checked current source, caller and model expectations.
async fn evaluate(
    service: &Service,
    token: &str,
    original: &Prepared,
    name: &str,
) -> Result<serde_json::Value, Failure> {
    if name.is_empty()
        || name.len() > 48
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(Failure::Input);
    }
    let current = prepare(service, token).await?;
    if !same_context(original, &current) {
        return Err(Failure::Unavailable);
    }
    let entry = current
        .catalog
        .entries
        .iter()
        .find(|entry| entry.pin.name == name)
        .cloned()
        .ok_or(Failure::Missing)?;
    let model = binding(&current, &entry)?.clone();
    service
        .limits
        .blocking(move || {
            let report = Store::open_env(&current.project.destination_database_env)
                .map_err(|_| Failure::Unavailable)?
                .read_reconciliation(
                    &current.project.plan,
                    &current.identity.caller,
                    current.source.lease().ok_or(Failure::Unavailable)?,
                    &model,
                    &entry.policy,
                )
                .map_err(|_| Failure::Unavailable)?;
            serde_json::to_value(report).map_err(|_| Failure::Unavailable)
        })
        .await
        .map_err(|_| Failure::Unavailable)?
}
/// Refresh grants, config and active revisions after all resolver work, before any response bytes escape.
async fn finish(
    service: &Service,
    token: &str,
    original: &Prepared,
    value: &impl Serialize,
) -> Response {
    let current = match prepare(service, token).await {
        Ok(current) => current,
        Err(failure) => return error(failure.status()),
    };
    if !same_context(original, &current) {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    }
    resources::guarded_json(value, || {
        // The final synchronous checks straddle bounded serialization, including file-policy changes.
        let Ok(project) = Project::load(&service.path) else {
            return false;
        };
        let Ok(identity) = auth::authenticate(&project, token) else {
            return false;
        };
        let Ok(catalog) = Catalog::load(project.reconciliation_file.as_deref()) else {
            return false;
        };
        identity.binding == current.identity.binding
            && catalog.fingerprint == current.catalog.fingerprint
            && project.plan.definition() == current.project.plan.definition()
            && full_scope(&project, &identity, &current.source).is_ok()
    })
}
