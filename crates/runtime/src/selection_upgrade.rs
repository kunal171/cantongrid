//! Operator-reviewed package handoff. Artifact inspection is offline; participant checks and
//! the fixed-boundary baseline are read-only. Only the owned destination is changed by apply.
use crate::{
    config::{Project, read},
    index::ready,
};
use cantongrid_core::{config::Plan, lifecycle::Batch};
use cantongrid_ledger::SourceAuthorization;
use cantongrid_models::Catalog;
use cantongrid_storage::{Store, UpgradeManifest, UpgradeResult, upgrade_boundary};
use std::{collections::BTreeSet, path::Path};

/// Explicit files are operator inputs, never request-controlled API paths.
pub struct UpgradeFiles<'a> {
    pub before: &'a Path,
    pub after: &'a Path,
    pub dar: &'a Path,
    pub models: &'a Path,
}
/// Validated local inputs plus package identities to verify on the configured participant.
struct Prepared {
    before: Project,
    after: Project,
    artifact: String,
    catalog: Catalog,
    packages: Vec<String>,
}
/// Resolve exact target templates from verified metadata; never infer a template from its short name.
fn prepare(files: &UpgradeFiles<'_>) -> Result<Prepared, String> {
    let before = Project::load(files.before)?;
    let after = Project::load(files.after)?;
    if before.destination_database_env != after.destination_database_env {
        return Err("upgrade must use the same owned destination".into());
    }
    let metadata = cantongrid_metadata::inspect_file(files.dar).map_err(|e| e.to_string())?;
    let catalog = Catalog::parse(
        std::str::from_utf8(&read(files.models, 65536)?).map_err(|_| "invalid upgrade models")?,
    )
    .map_err(|e| e.to_string())?;
    let added: Vec<_> = after
        .plan
        .templates()
        .filter(|t| !before.plan.selects(t))
        .collect();
    if added.is_empty() || before.plan.templates().any(|t| !after.plan.selects(t)) {
        return Err("upgrade must retain all old templates and add new ones".into());
    }
    for target in &added {
        if !metadata
            .packages
            .iter()
            .flat_map(|p| p.templates.iter())
            .any(|t| {
                t.id.package_id == target.package_id
                    && t.id.module == target.module
                    && t.id.name == target.entity
            })
        {
            return Err("added template is absent from verified DAR metadata".into());
        }
    }
    if catalog
        .models()
        .iter()
        .any(|m| !added.contains(&&m.template))
    {
        return Err("upgrade models must select only added templates".into());
    }
    let packages = added
        .iter()
        .map(|t| t.package_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Prepared {
        before,
        after,
        artifact: metadata.artifact_sha256,
        catalog,
        packages,
    })
}
/// Bracket package availability with exact current rights checks, then acquire a fresh short lease.
async fn authorize(prepared: &Prepared) -> Result<SourceAuthorization, String> {
    let source = &prepared.before.authorization;
    source
        .checker()
        .map_err(|e| e.to_string())?
        .require_packages_env(source.credential_env(), &prepared.packages)
        .await
        .map_err(|e| e.to_string())?;
    let mut auth = SourceAuthorization::new(source.checker().map_err(|e| e.to_string())?);
    auth.refresh_env(source.credential_env())
        .await
        .map_err(|e| e.to_string())?;
    ready(&prepared.before).await?;
    Ok(auth)
}
/// Emit a read-only manifest against the current acknowledged checkpoint. Register/install targets first.
pub async fn plan_upgrade(
    files: UpgradeFiles<'_>,
    namespace: &str,
) -> Result<UpgradeManifest, String> {
    let prepared = prepare(&files)?;
    let auth = authorize(&prepared).await?;
    let namespace = namespace.to_owned();
    tokio::task::spawn_blocking(move || {
        let targets: Vec<_> = prepared
            .catalog
            .models()
            .iter()
            .map(|m| (namespace.clone(), m.name.clone(), m.revision))
            .collect();
        let manifest = Store::open_env(&prepared.before.destination_database_env)
            .map_err(|e| e.to_string())?
            .plan_selection_upgrade(
                &prepared.before.plan,
                &prepared.after.plan,
                auth.lease().ok_or("source lease unavailable")?,
                &prepared.artifact,
                &targets,
            )
            .map_err(|e| e.to_string())?;
        verify_manifest(&prepared, &manifest)?;
        Ok(manifest)
    })
    .await
    .map_err(|_| "upgrade planning worker failed")?
}
/// Read a complete new-template baseline at the old completed boundary, not today's latest head.
/// If 65 rows exist, the reader's lookahead leaves snapshot=true and no mutation is attempted.
fn baseline(
    prepared: &Prepared,
    auth: &SourceAuthorization,
    manifest: &UpgradeManifest,
) -> Result<Batch, String> {
    let boundary = upgrade_boundary(&manifest.checkpoint).map_err(|e| e.to_string())?;
    let cursor = cantongrid_pqs::Cursor {
        snapshot: true,
        boundary,
        through: boundary,
        offset: boundary,
        pk: 0,
    };
    let checkpoint = serde_json::to_string(&cursor).map_err(|_| "invalid upgrade boundary")?;
    let batch = baseline_page(prepared, auth, &checkpoint, 64)?;
    let cursor: cantongrid_pqs::Cursor =
        serde_json::from_str(&batch.position).map_err(|_| "invalid upgrade baseline")?;
    if cursor.snapshot || cursor.boundary != boundary {
        return Err("upgrade baseline exceeds 64 contracts; no state changed".into());
    }
    Ok(batch)
}
/// Apply only the exact reviewed DAR, model fingerprints, source definitions and progress.
pub async fn apply_upgrade(
    files: UpgradeFiles<'_>,
    manifest_path: &Path,
) -> Result<UpgradeResult, String> {
    let prepared = prepare(&files)?;
    let manifest: UpgradeManifest = cantongrid_core::document::parse(
        std::str::from_utf8(&read(manifest_path, 65536)?)
            .map_err(|_| "invalid upgrade manifest")?,
    )
    .map_err(|_| "invalid upgrade manifest")?;
    verify_manifest(&prepared, &manifest)?;
    let auth = authorize(&prepared).await?;
    tokio::task::spawn_blocking(move || {
        let mut store = Store::open_env(&prepared.before.destination_database_env)
            .map_err(|e| e.to_string())?;
        // A committed replay needs no historical source snapshot, which may have been pruned since.
        let batch = if store
            .selection_upgrade_committed(&prepared.after.plan, &manifest)
            .map_err(|e| e.to_string())?
        {
            Batch {
                context: prepared.before.plan.context().clone(),
                position: String::new(),
                events: vec![],
            }
        } else {
            baseline(&prepared, &auth, &manifest)?
        };
        store
            .apply_selection_upgrade(
                &prepared.before.plan,
                &prepared.after.plan,
                auth.lease().ok_or("source lease unavailable")?,
                &manifest,
                &batch,
            )
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "upgrade application worker failed")?
}

/// Reuse the normal restricted PQS reader with an exact fixed-boundary snapshot cursor.
fn baseline_page(
    prepared: &Prepared,
    auth: &SourceAuthorization,
    checkpoint: &str,
    limit: usize,
) -> Result<Batch, String> {
    let selected = prepared
        .after
        .plan
        .templates()
        .filter(|t| !prepared.before.plan.selects(t))
        .cloned()
        .collect();
    let plan = Plan::pqs(
        prepared.before.plan.context().clone(),
        prepared.before.plan.source_parties().cloned().collect(),
        selected,
    )
    .map_err(|_| "invalid added-template plan")?;
    cantongrid_pqs::page_env(
        &prepared.before.source_database_env,
        &plan,
        auth.lease().ok_or("source lease unavailable")?,
        Some(checkpoint),
        limit,
    )
    .map_err(|e| e.to_string())?
    .ok_or("upgrade source page unavailable".into())
}

/// Execute one durable operator transition. Re-running step/status reconstructs progress from SQL;
/// no in-memory cursor or saved authorization lease is trusted across processes.
pub async fn upgrade_job(
    files: UpgradeFiles<'_>,
    manifest_path: &Path,
    action: &str,
) -> Result<cantongrid_storage::UpgradeStatus, String> {
    if !["start", "step", "status", "activate", "cancel"].contains(&action) {
        return Err("unsupported upgrade action".into());
    }
    let prepared = prepare(&files)?;
    let manifest: UpgradeManifest = cantongrid_core::document::parse(
        std::str::from_utf8(&read(manifest_path, 65536)?)
            .map_err(|_| "invalid upgrade manifest")?,
    )
    .map_err(|_| "invalid upgrade manifest")?;
    verify_manifest(&prepared, &manifest)?;
    let auth = authorize(&prepared).await?;
    let action = action.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut store = Store::open_env(&prepared.before.destination_database_env)
            .map_err(|e| e.to_string())?;
        let before = &prepared.before.plan;
        let after = &prepared.after.plan;
        let lease = auth.lease().ok_or("source lease unavailable")?;
        let result = match action.as_str() {
            "start" => store.start_selection_upgrade(before, after, lease, &manifest),
            "status" => store.selection_upgrade_status(before, lease, &manifest),
            "cancel" => store.cancel_selection_upgrade(before, lease, &manifest),
            other => {
                let state = store
                    .selection_upgrade_status(before, lease, &manifest)
                    .map_err(|e| e.to_string())?;
                if other == "step" && state.phase == "snapshot" {
                    let batch = baseline_page(
                        &prepared,
                        &auth,
                        &state.source_cursor,
                        prepared.before.page_size,
                    )?;
                    store.stage_selection_upgrade(
                        before,
                        lease,
                        &manifest,
                        &state.source_cursor,
                        &batch,
                    )
                } else {
                    // Re-probe the acknowledged boundary before publication. Pruned/regressed
                    // history fails explicitly; never resume by substituting the current head.
                    if (other == "activate" && state.phase == "ready")
                        || state.phase == "publishing"
                    {
                        baseline_page(&prepared, &auth, &manifest.checkpoint, 1)?;
                    }
                    if other == "activate" {
                        store.activate_selection_upgrade(before, after, lease, &manifest)
                    } else {
                        store.step_selection_upgrade(before, after, lease, &manifest)
                    }
                }
            }
        };
        result.map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "upgrade job worker failed")?
}

/// The reviewed artifact, selections and each model fingerprint must match the supplied inputs.
fn verify_manifest(prepared: &Prepared, manifest: &UpgradeManifest) -> Result<(), String> {
    if manifest.format_version != 1
        || manifest.before != prepared.before.plan.definition()
        || manifest.after != prepared.after.plan.definition()
        || manifest.artifact_sha256 != prepared.artifact
        || manifest.bindings.len() != prepared.catalog.models().len()
        || prepared.catalog.models().iter().any(|m| {
            !manifest.bindings.iter().any(|b| {
                b.name == m.name
                    && b.revision == m.revision
                    && m.sql_plan().is_ok_and(|s| s.fingerprint == b.fingerprint)
            })
        })
    {
        return Err("upgrade artifacts differ from reviewed manifest".into());
    }
    Ok(())
}
