//! Strict definitions compile once; private compiled nodes are never deserialized as trusted plans.
use crate::ModelError;
use cantongrid_core::{config::TemplateId, value::Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Exact output types; optionality applies to one scalar and never implies zero/default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldType {
    Text,
    Party,
    Bool,
    Int64,
    Numeric { scale: u8 },
    Optional { item: Box<FieldType> },
}

/// Paths are object-only; arbitrary expressions, array indexing and executable plugins are absent.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expression {
    Field {
        path: Vec<String>,
        #[serde(rename = "as")]
        value_type: FieldType,
    },
    Literal {
        value: Value,
    },
    Add {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Subtract {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Multiply {
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub field: String,
    pub equals: Value,
}

/// Source selection and output expressions are declarative; no source rights are implied.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub revision: u32,
    pub template: TemplateId,
    pub fields: BTreeMap<String, Expression>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    models: Vec<Model>,
}

/// Validated immutable definition set, sorted independently from developer JSON key order.
pub struct Catalog {
    models: Vec<Model>,
    fingerprint: String,
}
impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Catalog([redacted])")
    }
}
impl fmt::Debug for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Model([redacted])")
    }
}
impl fmt::Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Expression([redacted])")
    }
}
impl fmt::Debug for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Filter([redacted])")
    }
}

impl Catalog {
    /// Reject ambiguous configuration before constructing an engine plan; never retain raw errors.
    pub fn parse(document: &str) -> Result<Self, ModelError> {
        if document.len() > 65536 {
            return Err(ModelError::Limit);
        }
        let mut doc: Document =
            cantongrid_core::document::parse(document).map_err(|_| ModelError::Document)?;
        if doc.version != 1 || doc.models.is_empty() || doc.models.len() > 16 {
            return Err(ModelError::Definition);
        }
        let mut names = BTreeSet::new();
        for model in &doc.models {
            if !names.insert(model.name.clone()) {
                return Err(ModelError::Definition);
            }
            model.validate()?;
        }
        doc.models.sort_by(|a, b| a.name.cmp(&b.name));
        let fingerprint = digest(&serde_json::to_string(&doc).map_err(|_| ModelError::Document)?);
        Ok(Self {
            models: doc.models,
            fingerprint,
        })
    }
    pub fn models(&self) -> &[Model] {
        &self.models
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

impl Model {
    /// Validate every expression and compare filters against the exact projected output type.
    pub fn validate(&self) -> Result<(), ModelError> {
        if !identifier(&self.name)
            || self.revision == 0
            || self.template.validate().is_err()
            || self.fields.is_empty()
            || self.fields.len() > 32
        {
            return Err(ModelError::Definition);
        }
        let mut nodes = 0;
        let mut types = BTreeMap::new();
        for (name, expression) in &self.fields {
            if !identifier(name) {
                return Err(ModelError::Definition);
            }
            types.insert(name, expression.output_type_at(0, &mut nodes)?);
        }
        if let Some(filter) = &self.filter {
            let kind = types.get(&filter.field).ok_or(ModelError::Definition)?;
            kind.accepts(&filter.equals)?;
        }
        Ok(())
    }
    /// Canonical definition fingerprint is independent from map ordering and input whitespace.
    pub fn fingerprint(&self) -> Result<String, ModelError> {
        self.validate()?;
        Ok(digest(
            &serde_json::to_string(self).map_err(|_| ModelError::Document)?,
        ))
    }
}

impl FieldType {
    pub fn validate(&self) -> Result<(), ModelError> {
        match self {
            Self::Numeric { scale } if *scale > 37 => Err(ModelError::Type),
            Self::Optional { item } if matches!(**item, Self::Optional { .. }) => {
                Err(ModelError::Type)
            }
            Self::Optional { item } => item.validate(),
            _ => Ok(()),
        }
    }
    /// Check a typed literal/output without converting through an untyped JSON number.
    pub fn accepts(&self, value: &Value) -> Result<(), ModelError> {
        self.validate()?;
        value.validate().map_err(|_| ModelError::Type)?;
        let valid = match (self, value) {
            (Self::Text, Value::Text(_))
            | (Self::Party, Value::Party(_))
            | (Self::Bool, Value::Bool(_))
            | (Self::Int64, Value::Int64(_)) => true,
            (Self::Numeric { scale }, Value::Numeric(decimal)) => *scale == decimal.scale,
            (Self::Optional { .. }, Value::Optional(None)) => true,
            (Self::Optional { item }, Value::Optional(Some(value))) => return item.accepts(value),
            _ => false,
        };
        if valid { Ok(()) } else { Err(ModelError::Type) }
    }
}

impl Expression {
    pub fn output_type(&self) -> Result<FieldType, ModelError> {
        self.output_type_at(0, &mut 0)
    }
    fn output_type_at(&self, depth: usize, nodes: &mut usize) -> Result<FieldType, ModelError> {
        *nodes += 1;
        if depth > 16 || *nodes > 256 {
            return Err(ModelError::Limit);
        }
        match self {
            Self::Field { path, value_type } => {
                if path.is_empty() || path.len() > 16 || path.iter().any(|p| !path_identifier(p)) {
                    return Err(ModelError::Path);
                }
                value_type.validate()?;
                Ok(value_type.clone())
            }
            Self::Literal { value } => {
                let kind = match value {
                    Value::Text(_) => FieldType::Text,
                    Value::Party(_) => FieldType::Party,
                    Value::Bool(_) => FieldType::Bool,
                    Value::Int64(_) => FieldType::Int64,
                    Value::Numeric(n) => FieldType::Numeric { scale: n.scale },
                    _ => return Err(ModelError::Type),
                };
                kind.accepts(value)?;
                Ok(kind)
            }
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right } => {
                let left = left.output_type_at(depth + 1, nodes)?;
                let right = right.output_type_at(depth + 1, nodes)?;
                match (&left, &right) {
                    (FieldType::Int64, FieldType::Int64) => Ok(left),
                    (FieldType::Numeric { scale: a }, FieldType::Numeric { scale: b })
                        if a == b && !matches!(self, Self::Multiply { .. }) =>
                    {
                        Ok(left)
                    }
                    (FieldType::Numeric { .. }, FieldType::Int64)
                        if matches!(self, Self::Multiply { .. }) =>
                    {
                        Ok(left)
                    }
                    (FieldType::Int64, FieldType::Numeric { .. })
                        if matches!(self, Self::Multiply { .. }) =>
                    {
                        Ok(right)
                    }
                    _ => Err(ModelError::Type),
                }
            }
        }
    }
}

/// Output names are used by SQL/GraphQL generators. Path segments have a separate policy.
fn identifier(name: &str) -> bool {
    path_identifier(name)
        && name.len() <= 48
        && ![
            "contractId",
            "createEventId",
            "provenance",
            "activeContracts",
            "lifecycleEvents",
            "items",
            "visibleTotal",
            "nextCursor",
        ]
        .contains(&name)
}
fn path_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
