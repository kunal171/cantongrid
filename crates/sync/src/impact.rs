//! Offline dependency analysis over verified signatures and developer-authored model rules.
//! This report names configuration fields, never ledger payloads or processing literals.
use crate::{Diagnostic, SyncError as Error, compare, resolve::Resolver};
use cantongrid_metadata::schema::{Metadata, TypeId};
use cantongrid_models::{Expression, FieldType, Measure, Model};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// A model-wide report is useful even when one dependency blocks bundle generation.
#[derive(Default, Serialize)]
pub struct ImpactReport {
    pub format_version: u32,
    pub models: Vec<ModelImpact>,
    pub application_steps: Vec<&'static str>,
}
/// Exact generated identities change with revisions, even if output column types stay the same.
#[derive(Serialize)]
pub struct Artifacts {
    pub revision: u32,
    pub fingerprint: String,
    pub sql_table: String,
    pub query_field: String,
    pub diagnostic_query_field: String,
    pub client_type: String,
    pub aggregate_query_field: Option<String>,
    pub relationship_query_fields: BTreeMap<String, String>,
    pub join_query_fields: BTreeMap<String, String>,
}
/// Rebuild requirements are proposals, not evidence that an installed projection was rebuilt.
#[derive(Serialize)]
pub struct ModelImpact {
    pub model: String,
    pub affected: bool,
    pub rebuild: &'static str,
    pub before: Artifacts,
    pub proposed: Artifacts,
    pub fields: Vec<FieldDependency>,
    pub dependencies: Vec<Dependency>,
}
/// Each declared path retains both exact scalar and LF semantics (including enum members).
#[derive(Serialize)]
pub struct FieldDependency {
    pub projection: String,
    pub path: Vec<String>,
    pub declared: FieldType,
    pub before: Option<Signature>,
    pub after: Option<Signature>,
    pub before_error: Option<Error>,
    pub after_error: Option<Error>,
    pub compatible: bool,
}
/// Resolved selected LF semantics, kept separate from the developer-declared output type.
#[derive(Serialize)]
pub struct Signature {
    pub scalar: FieldType,
    pub semantics: serde_json::Value,
}
/// Directed edges refer to projected columns. Consumers can trace a sum or join back to LF paths.
#[derive(Serialize)]
pub struct Dependency {
    pub kind: &'static str,
    pub name: String,
    pub model: String,
    pub fields: Vec<String>,
    pub target_revision: Option<u32>,
    pub proposed_target_revision: Option<u32>,
    pub review: bool,
}

/// Gather arithmetic operands without inspecting literal values. Bounds come from Model::validate.
fn reads(expression: &Expression, out: &mut Vec<(Vec<String>, FieldType)>) {
    match expression {
        Expression::Field { path, value_type } => {
            out.push((path.clone(), value_type.clone()));
        }
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right } => {
            reads(left, out);
            reads(right, out);
        }
        Expression::Literal { .. } => {}
    }
}
/// A finite resolver failure remains a report finding; exhaustion aborts report construction.
fn signature(
    resolver: &mut Resolver<'_>,
    template: &TypeId,
    path: &[String],
    remaining: &mut usize,
) -> Result<(Option<Signature>, Option<Error>), Error> {
    match resolver.field(template, path) {
        Ok(selected) => {
            // Charge serialized metadata before retaining it. Repeated wide enums must not
            // multiply a bounded DAR into an unbounded per-expression report.
            let bytes = serde_json::to_vec(&selected.semantics)
                .map_err(|_| Error::Limit)?
                .len();
            *remaining = remaining.checked_sub(bytes).ok_or(Error::Limit)?;
            Ok((
                Some(Signature {
                    scalar: selected.scalar,
                    semantics: selected.semantics,
                }),
                None,
            ))
        }
        Err(Error::Limit) => Err(Error::Limit),
        Err(error) => Ok((None, Some(error))),
    }
}
/// Use the same identity functions as runtime/client generation; unresolved joins stay absent.
fn artifacts(model: &Model, models: &[Model]) -> Result<Artifacts, Error> {
    let compile = || -> Result<Artifacts, cantongrid_models::ModelError> {
        let mut relationships = BTreeMap::new();
        let mut joins = BTreeMap::new();
        for link in &model.relationships {
            relationships.insert(link.name.clone(), model.relationship_api_name(&link.name)?);
            if link.join.is_some()
                && let Some(target) = models.iter().find(|m| m.name == link.target.model)
                && let Ok(name) = model.join_api_name(&link.name, target)
            {
                joins.insert(link.name.clone(), name);
            }
        }
        Ok(Artifacts {
            revision: model.revision,
            fingerprint: model.fingerprint()?,
            sql_table: model.sql_plan()?.table,
            query_field: model.api_name()?,
            diagnostic_query_field: model.diagnostic_api_name()?,
            client_type: model.api_type()?,
            aggregate_query_field: model
                .aggregation
                .as_ref()
                .map(|_| model.aggregate_api_name())
                .transpose()?,
            relationship_query_fields: relationships,
            join_query_fields: joins,
        })
    };
    compile().map_err(|_| Error::Definition)
}
/// Explain downstream consumers without rewriting policies. An external target must be reviewed
/// against its registered binding; an in-catalog target revision change is a hard sync blocker.
fn dependencies(model: &Model, proposed: &[Model]) -> Vec<Dependency> {
    let local = |kind, name: String, fields: Vec<String>| Dependency {
        kind,
        name,
        model: model.name.clone(),
        fields,
        target_revision: None,
        proposed_target_revision: None,
        review: false,
    };
    let mut result = Vec::new();
    if let Some(filter) = &model.filter {
        result.push(local("filter", "filter".into(), vec![filter.field.clone()]));
    }
    if let Some(aggregate) = &model.aggregation {
        result.push(local(
            "aggregate_group",
            "group_by".into(),
            aggregate.group_by.clone(),
        ));
        for (name, measure) in &aggregate.measures {
            // Count depends on row membership, hence all projected inputs and the model filter.
            let fields = match measure {
                Measure::Count {} => model.fields.keys().cloned().collect(),
                Measure::Sum { field } => vec![field.clone()],
            };
            result.push(local("aggregate_measure", name.clone(), fields));
        }
    }
    let mut links: Vec<_> = model.relationships.iter().collect();
    links.sort_by(|a, b| a.name.cmp(&b.name));
    for link in links {
        let target = proposed.iter().find(|m| m.name == link.target.model);
        let review = target.is_none_or(|m| m.revision != link.target.revision);
        result.push(local(
            "relationship_source",
            link.name.clone(),
            vec![link.source_field.clone()],
        ));
        let mut target_fields = BTreeSet::from([link.target.field.clone()]);
        if let Some(join) = &link.join {
            result.push(local(
                "join_source",
                link.name.clone(),
                join.source_fields.clone(),
            ));
            target_fields.extend(join.target_fields.iter().cloned());
        }
        result.push(Dependency {
            kind: if link.join.is_some() {
                "join_target"
            } else {
                "relationship_target"
            },
            name: link.name.clone(),
            model: link.target.model.clone(),
            fields: target_fields.into_iter().collect(),
            target_revision: Some(link.target.revision),
            proposed_target_revision: target.map(|m| m.revision),
            review,
        });
    }
    result
}
/// Analyze each selected read independently. Nominal package IDs normalize only for comparison;
/// the report preserves original signatures so an operator can inspect the exact artifact change.
pub(crate) fn build(
    old: &Metadata,
    new: &Metadata,
    previous: &[Model],
    proposed: &[Model],
    diagnostics: &[Diagnostic],
) -> Result<ImpactReport, Error> {
    let mappings = old
        .packages
        .iter()
        .filter_map(|p| {
            compare::replacement(p, new)
                .ok()
                .map(|n| (p.package_id.clone(), n.package_id.clone()))
        })
        .collect();
    let mut old_resolver = Resolver::new(old);
    let mut new_resolver = Resolver::new(new);
    let mut models = Vec::new();
    let mut remaining = 4 * 1024 * 1024;
    for (model, candidate) in previous.iter().zip(proposed) {
        let before_id = TypeId {
            package_id: model.template.package_id.clone(),
            module: model.template.module.clone(),
            name: model.template.entity.clone(),
        };
        // A failed identity mapping must not inspect the old package retained in a new DAR.
        let after_id = old
            .packages
            .iter()
            .find(|p| p.package_id == model.template.package_id)
            .and_then(|p| compare::replacement(p, new).ok())
            .map(|p| TypeId {
                package_id: p.package_id.clone(),
                ..before_id.clone()
            });
        let mut fields = Vec::new();
        for (name, expression) in &model.fields {
            let mut paths = Vec::new();
            reads(expression, &mut paths);
            paths.sort_by(|a, b| a.0.cmp(&b.0));
            paths.dedup();
            for (path, declared) in paths {
                let (before, before_error) =
                    signature(&mut old_resolver, &before_id, &path, &mut remaining)?;
                let (after, after_error) = match &after_id {
                    Some(id) => signature(&mut new_resolver, id, &path, &mut remaining)?,
                    None => (None, Some(Error::Identity)),
                };
                let compatible = match (&before, &after) {
                    (Some(a), Some(b)) => {
                        let mut semantics = a.semantics.clone();
                        compare::normalize(&mut semantics, &mappings);
                        a.scalar == declared && b.scalar == declared && semantics == b.semantics
                    }
                    _ => false,
                };
                fields.push(FieldDependency {
                    projection: name.clone(),
                    path,
                    declared,
                    before,
                    after,
                    before_error,
                    after_error,
                    compatible,
                });
            }
        }
        let before = artifacts(model, previous)?;
        let after = artifacts(candidate, proposed)?;
        let blocked = diagnostics.iter().any(|d| d.model == model.name);
        let affected = blocked || before.fingerprint != after.fingerprint;
        models.push(ModelImpact {
            model: model.name.clone(),
            affected,
            rebuild: if blocked {
                "blocked"
            } else if affected {
                "required"
            } else {
                "unnecessary"
            },
            before,
            proposed: after,
            fields,
            dependencies: dependencies(model, proposed),
        });
    }
    Ok(ImpactReport {
        format_version: 1,
        models,
        application_steps: vec![
            "review_report_and_relationship_pins",
            "register_and_install_required_models",
            "rebuild_and_verify_before_promotion",
            "obtain_current_source_authorized_upgrade_plan",
            "retain_checkpoints_history_and_previous_templates",
        ],
    })
}

impl crate::SyncPlan {
    /// Concise operator output, also included in compatible bundles. Findings never apply changes.
    pub fn impact_summary(&self) -> String {
        let mut text = format!(
            "Upgrade impact (offline)\nCompatible: {}\n",
            self.report.compatible
        );
        for model in &self.report.impact.models {
            text.push_str(&format!(
                "- {}: rebuild {}; revision {} -> {}\n",
                model.model, model.rebuild, model.before.revision, model.proposed.revision
            ));
            for dependency in model.dependencies.iter().filter(|d| d.review) {
                text.push_str(&format!(
                    "  Review {} {} -> {} (pinned {}, proposed {})\n",
                    dependency.kind,
                    dependency.name,
                    dependency.model,
                    dependency
                        .target_revision
                        .map_or_else(|| "unknown".into(), |n| n.to_string()),
                    dependency
                        .proposed_target_revision
                        .map_or_else(|| "external".into(), |n| n.to_string())
                ));
            }
        }
        text.push_str("No ledger, database, checkpoint, history or grant changes applied.\n");
        text
    }
}
