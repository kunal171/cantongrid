//! Pure per-contract projection. Caller authorization belongs to the storage/API boundary.
use crate::{
    Expression, FieldType, Model, ModelError,
    arithmetic::{self, Operation},
};
use cantongrid_core::{value::Value, visibility::PartyId};
use std::collections::BTreeMap;

impl Model {
    /// Evaluate one raw canonical payload deterministically. A false filter returns None;
    /// invalid/missing values are errors, never a fabricated empty row or zero balance.
    pub fn evaluate(&self, payload: &str) -> Result<Option<BTreeMap<String, Value>>, ModelError> {
        self.validate()?;
        if payload.len() > 65536 {
            return Err(ModelError::Limit);
        }
        let root: serde_json::Value =
            cantongrid_core::document::parse(payload).map_err(|_| ModelError::Type)?;
        if !root.is_object() {
            return Err(ModelError::Type);
        }
        let fields: BTreeMap<_, _> = self
            .fields
            .iter()
            .map(|(name, expr)| evaluate(expr, &root).map(|value| (name.clone(), value)))
            .collect::<Result<_, _>>()?;
        if serde_json::to_vec(&fields)
            .map_err(|_| ModelError::Type)?
            .len()
            > 1024 * 1024
        {
            return Err(ModelError::Limit);
        }
        if self
            .filter
            .as_ref()
            .is_some_and(|filter| fields.get(&filter.field) != Some(&filter.equals))
        {
            return Ok(None);
        }
        Ok(Some(fields))
    }
}

/// Resolve only validated object-field paths; an unexpected scalar ancestor is a type error.
fn evaluate(expression: &Expression, root: &serde_json::Value) -> Result<Value, ModelError> {
    match expression {
        Expression::Literal { value } => Ok(value.clone()),
        Expression::Field { path, value_type } => {
            let mut current = Some(root);
            for segment in path {
                current = match current {
                    None | Some(serde_json::Value::Null) => None,
                    Some(serde_json::Value::Object(fields)) => fields.get(segment),
                    _ => return Err(ModelError::Type),
                };
            }
            convert(value_type, current)
        }
        Expression::Add { left, right } => arithmetic::calculate(
            Operation::Add,
            evaluate(left, root)?,
            evaluate(right, root)?,
        ),
        Expression::Subtract { left, right } => arithmetic::calculate(
            Operation::Subtract,
            evaluate(left, root)?,
            evaluate(right, root)?,
        ),
        Expression::Multiply { left, right } => arithmetic::calculate(
            Operation::Multiply,
            evaluate(left, root)?,
            evaluate(right, root)?,
        ),
    }
}

/// Preserve source financial strings, applying only exact declared-scale normalization.
fn convert(kind: &FieldType, input: Option<&serde_json::Value>) -> Result<Value, ModelError> {
    if let FieldType::Optional { item } = kind {
        return match input {
            None | Some(serde_json::Value::Null) => Ok(Value::Optional(None)),
            Some(input) => {
                convert(item, Some(input)).map(|value| Value::Optional(Some(Box::new(value))))
            }
        };
    }
    let input = input.filter(|v| !v.is_null()).ok_or(ModelError::Missing)?;
    let text = || input.as_str().ok_or(ModelError::Type);
    let value = match kind {
        FieldType::Text => Value::Text(text()?.into()),
        FieldType::Party => {
            let value = text()?;
            PartyId::new(value).map_err(|_| ModelError::Type)?;
            Value::Party(value.into())
        }
        FieldType::Bool => Value::Bool(input.as_bool().ok_or(ModelError::Type)?),
        FieldType::Int64 => Value::Int64(text()?.into()),
        FieldType::Numeric { scale } => Value::Numeric(arithmetic::decimal(text()?, *scale)?),
        FieldType::Optional { .. } => return Err(ModelError::Type),
    };
    kind.accepts(&value)?;
    Ok(value)
}
