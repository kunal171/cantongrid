//! Explicit inner-join projections never broaden canonical input visibility.
use crate::{FieldType, Model, ModelError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Only inner equality joins are supported; unmatched/hidden inputs produce no placeholder row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinKind {
    Inner,
}

/// Selected model columns, not payload paths or request-supplied expressions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinProjection {
    pub kind: JoinKind,
    pub source_fields: Vec<String>,
    pub target_fields: Vec<String>,
}

/// Validate bounded, duplicate-free column selections against their exact registered model.
fn fields(model: &Model, names: &[String]) -> Result<BTreeMap<String, FieldType>, ModelError> {
    let unique: BTreeSet<_> = names.iter().collect();
    if names.is_empty() || names.len() > 16 || unique.len() != names.len() {
        return Err(ModelError::Definition);
    }
    names
        .iter()
        .map(|name| {
            Ok((
                name.clone(),
                model
                    .fields
                    .get(name)
                    .ok_or(ModelError::Definition)?
                    .output_type()?,
            ))
        })
        .collect()
}

impl JoinProjection {
    /// Target field existence is checked once its registered model is resolved.
    pub(crate) fn validate_source(&self, model: &Model) -> Result<(), ModelError> {
        fields(model, &self.source_fields)?;
        if self.target_fields.is_empty()
            || self.target_fields.len() > 16
            || self.target_fields.iter().collect::<BTreeSet<_>>().len() != self.target_fields.len()
            || self
                .target_fields
                .iter()
                .any(|name| !crate::definition::identifier(name))
        {
            return Err(ModelError::Definition);
        }
        Ok(())
    }
    /// Retain exact output types including bool/null; optional types are outputs, never equality keys.
    pub(crate) fn validate_target(&self, target: &Model) -> Result<(), ModelError> {
        fields(target, &self.target_fields).map(|_| ())
    }
}

/// Exact selected field vocabulary used by SQL, GraphQL and TypeScript generators.
pub type JoinFields = (BTreeMap<String, FieldType>, BTreeMap<String, FieldType>);
impl Model {
    /// Validate key policy plus both projections; this never authenticates either endpoint.
    pub fn join_fields(&self, name: &str, target: &Model) -> Result<JoinFields, ModelError> {
        self.validate_relationship(name, target)?;
        let projection = self
            .relationship(name)?
            .join
            .as_ref()
            .ok_or(ModelError::Definition)?;
        Ok((
            fields(self, &projection.source_fields)?,
            fields(target, &projection.target_fields)?,
        ))
    }
    /// Include the target fingerprint: identically named revisions in distinct namespaces may differ.
    pub fn join_api_name(&self, name: &str, target: &Model) -> Result<String, ModelError> {
        self.join_fields(name, target)?;
        Ok(format!(
            "{}_join_{}_{}",
            self.api_name()?,
            name,
            &target.fingerprint()?[..16]
        ))
    }
    /// Join output types have their own domain and cannot collide with relationship or model rows.
    pub fn join_api_type(&self, name: &str, target: &Model) -> Result<String, ModelError> {
        self.join_fields(name, target)?;
        Ok(format!(
            "{}Join_{}_{}",
            self.api_type()?,
            name,
            &target.fingerprint()?[..16]
        ))
    }
}
