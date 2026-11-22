//! Compare selected fields before producing any artifact. Expressions/literals, filters and
//! aggregation rules are retained verbatim; only unambiguous template IDs/revisions are refreshed.
use crate::{SyncError as Error, compare, resolve::Resolver};
use cantongrid_metadata::schema::{Metadata, TypeId};
use cantongrid_models::{Catalog, Expression};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Diagnostics name the developer's projection, never an input literal or private source payload.
#[derive(Serialize)]
pub struct Diagnostic {
    pub model: String,
    pub field: Option<String>,
    pub error: Error,
}
/// The package-wide diff and selected projection compatibility remain separate decisions.
#[derive(Serialize)]
pub struct Report {
    pub format_version: u32,
    pub old_artifact_sha256: String,
    pub new_artifact_sha256: String,
    pub compatible: bool,
    pub changes: Vec<compare::Change>,
    pub packages_added: Vec<String>,
    pub packages_removed: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub impact: crate::ImpactReport,
}
/// A source-only review result. A blocked result has no generated model catalog to consume.
pub struct SyncPlan {
    pub report: Report,
    pub(crate) catalog: Option<Catalog>,
    pub(crate) metadata: String,
    pub(crate) previous: Vec<cantongrid_models::Model>,
}
/// Visit only declared field reads, including arithmetic operands. The existing model validator
/// already bounds expression depth/nodes and enforces exact arithmetic output rules.
fn fields<'a>(
    expression: &'a Expression,
    reads: &mut Vec<(&'a [String], &'a cantongrid_models::FieldType)>,
) {
    match expression {
        Expression::Field { path, value_type } => reads.push((path, value_type)),
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right } => {
            fields(left, reads);
            fields(right, reads);
        }
        Expression::Literal { .. } => {}
    }
}
/// Preserve unsupported language-version diagnostics without forwarding archive internals/paths.
fn metadata_error(error: cantongrid_metadata::MetadataError) -> Error {
    if error == cantongrid_metadata::MetadataError::UnsupportedVersion {
        Error::UnsupportedVersion
    } else {
        Error::Metadata
    }
}
/// The public entry verifies original DAR hashes and strict model definitions before comparison.
pub fn plan(old_dar: &[u8], new_dar: &[u8], models_json: &str) -> Result<SyncPlan, Error> {
    let old = cantongrid_metadata::inspect(old_dar).map_err(metadata_error)?;
    let new = cantongrid_metadata::inspect(new_dar).map_err(metadata_error)?;
    let catalog = Catalog::parse(models_json).map_err(|_| Error::Definition)?;
    analyze(&old, &new, &catalog)
}
/// Internal structured comparison is shared with synthetic conformance tests; production callers
/// enter through verified DAR bytes, not arbitrary deserialized signature graphs.
pub(crate) fn analyze(
    old: &Metadata,
    new: &Metadata,
    catalog: &Catalog,
) -> Result<SyncPlan, Error> {
    let mut mappings = BTreeMap::new();
    for package in &old.packages {
        if let Ok(target) = compare::replacement(package, new) {
            mappings.insert(package.package_id.clone(), target.package_id.clone());
        }
    }
    let mut report = Report {
        format_version: 1,
        old_artifact_sha256: old.artifact_sha256.clone(),
        new_artifact_sha256: new.artifact_sha256.clone(),
        compatible: true,
        changes: vec![],
        packages_added: new
            .packages
            .iter()
            .filter(|p| {
                !old.packages
                    .iter()
                    .any(|old| old.package_id == p.package_id)
            })
            .map(|p| p.package_id.clone())
            .collect(),
        packages_removed: old
            .packages
            .iter()
            .filter(|p| {
                !new.packages
                    .iter()
                    .any(|new| new.package_id == p.package_id)
            })
            .map(|p| p.package_id.clone())
            .collect(),
        diagnostics: vec![],
        impact: crate::ImpactReport::default(),
    };
    let mut old_resolver = Resolver::new(old);
    let mut new_resolver = Resolver::new(new);
    for package in &old.packages {
        if let Ok(target) = compare::replacement(package, new) {
            report
                .changes
                .extend(compare::changes(package, target, &mappings)?);
            if report.changes.len() > 4096 {
                return Err(Error::Limit);
            }
        }
    }

    let mut models = catalog.models().to_vec();
    let mut families: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for model in catalog.models() {
        if let Some(p) = old
            .packages
            .iter()
            .find(|p| p.package_id == model.template.package_id)
        {
            families.entry(&p.name).or_default().insert(&p.package_id);
        }
    }
    for model in &mut models {
        let old_package = old
            .packages
            .iter()
            .find(|p| p.package_id == model.template.package_id);
        let Some(old_package) = old_package else {
            report.diagnostics.push(Diagnostic {
                model: model.name.clone(),
                field: None,
                error: Error::Identity,
            });
            continue;
        };
        if families
            .get(old_package.name.as_str())
            .is_some_and(|ids| ids.len() > 1)
        {
            report.diagnostics.push(Diagnostic {
                model: model.name.clone(),
                field: None,
                error: Error::Identity,
            });
            continue;
        }
        let target = match compare::replacement(old_package, new) {
            Ok(p) => p,
            Err(error) => {
                report.diagnostics.push(Diagnostic {
                    model: model.name.clone(),
                    field: None,
                    error,
                });
                continue;
            }
        };
        let before = TypeId {
            package_id: model.template.package_id.clone(),
            module: model.template.module.clone(),
            name: model.template.entity.clone(),
        };
        let after = TypeId {
            package_id: target.package_id.clone(),
            module: before.module.clone(),
            name: before.name.clone(),
        };
        if !old_package.templates.iter().any(|t| t.id == before)
            || !target.templates.iter().any(|t| t.id == after)
        {
            report.diagnostics.push(Diagnostic {
                model: model.name.clone(),
                field: None,
                error: Error::Identity,
            });
            continue;
        }
        for (name, expression) in &model.fields {
            let mut reads = Vec::new();
            fields(expression, &mut reads);
            for (path, declared) in reads {
                let checked = (|| {
                    let mut a = old_resolver.field(&before, path)?;
                    let b = new_resolver.field(&after, path)?;
                    compare::normalize(&mut a.semantics, &mappings);
                    if &a.scalar != declared || &b.scalar != declared || a.semantics != b.semantics
                    {
                        return Err(Error::Incompatible);
                    }
                    Ok(())
                })();
                if let Err(error) = checked {
                    report.diagnostics.push(Diagnostic {
                        model: model.name.clone(),
                        field: Some(name.clone()),
                        error,
                    });
                }
            }
        }
        if model.template.package_id != target.package_id {
            model.template.package_id = target.package_id.clone();
            model.revision = model.revision.checked_add(1).ok_or(Error::Limit)?;
        }
    }
    // Target revisions are application policy, not package identity. Never rewrite a pin.
    // Keep the report available when refreshed models would fail catalog cross-validation.
    for model in &models {
        for link in &model.relationships {
            let target = models.iter().find(|m| m.name == link.target.model);
            let error = match target {
                Some(target) if target.revision != link.target.revision => {
                    Some(Error::Incompatible)
                }
                None if link.join.is_some() => Some(Error::Identity),
                _ => None,
            };
            if let Some(error) = error {
                report.diagnostics.push(Diagnostic {
                    model: model.name.clone(),
                    field: Some(format!("relationship.{}", link.name)),
                    error,
                });
            }
        }
    }
    report.impact = crate::impact::build(old, new, catalog.models(), &models, &report.diagnostics)?;
    if report.changes.len() > 4096 || report.diagnostics.len() > 8192 {
        return Err(Error::Limit);
    }
    report
        .changes
        .sort_by(|a, b| (&a.package, &a.declaration).cmp(&(&b.package, &b.declaration)));
    report.compatible = report.diagnostics.is_empty();
    let refreshed = if report.compatible {
        Some(
            Catalog::parse(&serde_json::json!({"version":1,"models":models}).to_string())
                .map_err(|_| Error::Definition)?,
        )
    } else {
        None
    };
    Ok(SyncPlan {
        report,
        catalog: refreshed,
        metadata: new.to_json().map_err(|_| Error::Limit)?,
        previous: catalog.models().to_vec(),
    })
}
