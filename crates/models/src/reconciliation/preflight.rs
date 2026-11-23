//! Check projected field compatibility without consulting an installed or active database binding.
use super::{Policy, PolicyIdentity};
use crate::{Expression, FieldType, Model, ModelError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Configured immutable model identity, not evidence of installation, activation or source selection.
#[derive(Debug, Serialize)]
struct ModelIdentity {
    namespace: String,
    name: String,
    revision: String,
    fingerprint: String,
}

/// Read-only schema diagnostic. Private fields and no Deserialize prevent supplied authorization claims.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreflight {
    format_version: u32,
    policy: PolicyIdentity,
    model: ModelIdentity,
    fingerprint: String,
    referenced_fields: usize,
    status: &'static str,
    source_access: &'static str,
    active_binding_checked: bool,
}

impl Policy {
    /// Resolve each policy leaf against projected output types, never the model's raw payload paths.
    /// Example: a model's amount may come from balances.amount; the policy still reads only amount.
    pub fn preflight_model(
        &self,
        namespace: &str,
        model: &Model,
    ) -> Result<ModelPreflight, ModelError> {
        // Match registry namespace syntax, including hyphens and its 64-byte bound.
        if namespace.is_empty()
            || namespace.len() > 64
            || !namespace.as_bytes()[0].is_ascii_alphabetic()
            || !namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(ModelError::Definition);
        }
        // Public Models can be changed by their owners: fingerprint() validates the complete model.
        let fingerprint = model.fingerprint()?;
        let types = model
            .fields
            .iter()
            .map(|(name, expression)| Ok((name.as_str(), expression.output_type()?)))
            .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
        let mut referenced = BTreeSet::new();
        let dependencies = self
            .definition
            .fields
            .iter()
            .map(|(name, expression)| Ok((name, references(expression, &types, &mut referenced)?)))
            .collect::<Result<BTreeMap<_, _>, ModelError>>()?;
        // An unused source expression must not disguise a constant-only check as model-dependent.
        if self
            .definition
            .checks
            .iter()
            .any(|check| !dependencies[&check.left] && !dependencies[&check.right])
        {
            return Err(ModelError::Definition);
        }
        let policy = PolicyIdentity {
            name: self.name().into(),
            revision: self.revision().to_string(),
            fingerprint: self.fingerprint().into(),
        };
        let model = ModelIdentity {
            namespace: namespace.into(),
            name: model.name.clone(),
            revision: model.revision.to_string(),
            fingerprint,
        };
        // Both immutable definitions and the namespace matter; catalog order does not.
        let canonical =
            serde_json::to_vec(&(1_u32, &policy, &model)).map_err(|_| ModelError::Document)?;
        let mut digest = Sha256::new();
        digest.update(b"cantongrid:reconciliation:model-preflight:v1\0");
        digest.update(canonical);
        Ok(ModelPreflight {
            format_version: 1,
            policy,
            model,
            fingerprint: digest
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            referenced_fields: referenced.len(),
            status: "schema_compatible",
            source_access: "not_performed",
            active_binding_checked: false,
        })
    }
}

/// Walk an already bounded policy tree, checking every leaf even in currently unused expressions.
fn references<'a>(
    expression: &'a Expression,
    types: &BTreeMap<&str, FieldType>,
    referenced: &mut BTreeSet<&'a str>,
) -> Result<bool, ModelError> {
    match expression {
        Expression::Field { path, value_type } => {
            let [name] = path.as_slice() else {
                return Err(ModelError::Path);
            };
            let kind = types.get(name.as_str()).ok_or(ModelError::Missing)?;
            if kind != value_type {
                return Err(ModelError::Type);
            }
            referenced.insert(name);
            Ok(true)
        }
        Expression::Literal { .. } => Ok(false),
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right } => {
            let left = references(left, types, referenced)?;
            let right = references(right, types, referenced)?;
            Ok(left || right)
        }
    }
}
