//! Compile a bounded immutable policy separately from untrusted fixture inputs.
use crate::{Expression, FieldType, ModelError, definition::identifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Closed exact-order relations; null tests, approximations and executable predicates are unsupported.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// Check operands refer only to declared expression names, never arbitrary SQL or payload paths.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Check {
    pub name: String,
    pub left: String,
    pub operator: Operator,
    pub right: String,
}
/// Private wire definition: callers cannot deserialize an allegedly compiled/authorized policy.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    pub name: String,
    pub revision: u32,
    pub fields: BTreeMap<String, Expression>,
    pub checks: Vec<Check>,
}
/// Version the entire policy grammar independently from the application's revision identifier.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    policy: Definition,
}

/// A compiled pure policy; this identity grants no source or caller rights.
pub struct Policy {
    pub(super) definition: Definition,
    fingerprint: String,
}
impl fmt::Debug for Policy {
    /// Configuration can contain private literals and field paths; keep Debug output redacted too.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Policy([redacted])")
    }
}
impl Policy {
    /// Reject ambiguous input and validate shared expression budgets before canonical fingerprinting.
    pub fn parse(text: &str) -> Result<Self, ModelError> {
        if text.len() > 65536 {
            return Err(ModelError::Limit);
        }
        let mut doc: Document =
            cantongrid_core::document::parse(text).map_err(|_| ModelError::Document)?;
        if doc.version != 1 {
            return Err(ModelError::Definition);
        }
        doc.policy.validate()?;
        doc.policy.checks.sort_by(|a, b| a.name.cmp(&b.name));
        let canonical = serde_json::to_vec(&doc).map_err(|_| ModelError::Document)?;
        // Domain separation prevents a policy receipt from being confused with a model fingerprint.
        let mut digest = Sha256::new();
        digest.update(b"cantongrid:reconciliation:policy:v1\0");
        digest.update(canonical);
        let fingerprint = digest
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self {
            definition: doc.policy,
            fingerprint,
        })
    }
    /// Names/revisions are configured metadata, not identifiers copied from a ledger payload.
    pub fn name(&self) -> &str {
        &self.definition.name
    }
    /// Policy revisions are positive explicit developer versions, independent of source offsets.
    pub fn revision(&self) -> u32 {
        self.definition.revision
    }
    /// Canonical definitions include the revision; input formatting and check order are immaterial.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Expose the finite configured check count without retaining or inspecting any source data.
    pub fn check_count(&self) -> usize {
        self.definition.checks.len()
    }
}
impl Definition {
    /// Numeric operands must agree exactly in type/scale; optional/missing values never mean zero.
    fn validate(&self) -> Result<(), ModelError> {
        if !identifier(&self.name)
            || self.revision == 0
            || self.fields.is_empty()
            || self.checks.is_empty()
        {
            return Err(ModelError::Definition);
        }
        if self.fields.len() > 32 || self.checks.len() > 16 {
            return Err(ModelError::Limit);
        }
        let mut nodes = 0;
        let mut types = BTreeMap::new();
        for (name, expression) in &self.fields {
            if !identifier(name) {
                return Err(ModelError::Definition);
            }
            let kind = expression.output_type_at(0, &mut nodes)?;
            if !matches!(kind, FieldType::Int64 | FieldType::Numeric { .. }) {
                return Err(ModelError::Type);
            }
            types.insert(name, kind);
        }
        let mut names = BTreeSet::new();
        for check in &self.checks {
            if !identifier(&check.name) || !names.insert(&check.name) {
                return Err(ModelError::Definition);
            }
            let left = types.get(&check.left).ok_or(ModelError::Definition)?;
            let right = types.get(&check.right).ok_or(ModelError::Definition)?;
            if left != right {
                return Err(ModelError::Type);
            }
        }
        Ok(())
    }
}
