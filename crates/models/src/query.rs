//! Bounded query vocabulary shared by SQL, REST and generated GraphQL. It never grants access.
use crate::{FieldType, Model, ModelError};
use cantongrid_core::value::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Every predicate is ANDed. Only exact strings and booleans cross the query boundary.
#[derive(Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryOptions {
    #[serde(default)]
    pub filters: BTreeMap<String, BTreeMap<Comparison, QueryValue>>,
    #[serde(default)]
    pub sort: Option<Sort>,
}

/// SQL operators are chosen from this closed enum, never from user-provided SQL fragments.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Comparison {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    IsNull,
}
impl Comparison {
    pub fn name(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Gt => "gt",
            Self::Gte => "gte",
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::IsNull => "isNull",
        }
    }
    pub fn is_range(self) -> bool {
        matches!(self, Self::Gt | Self::Gte | Self::Lt | Self::Lte)
    }
}

/// JSON numbers, arrays, objects and null are invalid operator arguments. Use isNull for nulls.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum QueryValue {
    Text(String),
    Bool(bool),
}
impl QueryValue {
    /// PostgreSQL receives only bound text parameters, then casts to the validated native type.
    pub fn parameter(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Bool(b) => b.to_string(),
        }
    }
}

/// Nulls sort last in both directions; contract ID ascending is always the unique tie breaker.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub field: String,
    #[serde(default)]
    pub direction: Direction,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Asc,
    Desc,
}

/// Normalized options are immutable. Their hash binds pagination to the complete query semantics.
#[derive(Clone)]
pub struct ValidatedQuery {
    options: QueryOptions,
    fingerprint: String,
}
impl ValidatedQuery {
    pub fn options(&self) -> &QueryOptions {
        &self.options
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Public schema metadata excludes expressions, source paths, literals and SQL identifiers.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryField {
    pub name: String,
    pub kind: &'static str,
    pub nullable: bool,
    pub scale: Option<u8>,
    pub operators: Vec<&'static str>,
}

impl FieldType {
    /// Equality is scalar-wide; ranges apply only to financial/integer values, not text or bool.
    pub fn query_operators(&self) -> Vec<Comparison> {
        let (kind, nullable) = self.query_scalar();
        let mut ops = vec![Comparison::Eq, Comparison::Ne];
        if matches!(kind, Self::Int64 | Self::Numeric { .. }) {
            ops.extend([
                Comparison::Gt,
                Comparison::Gte,
                Comparison::Lt,
                Comparison::Lte,
            ]);
        }
        if nullable {
            ops.push(Comparison::IsNull);
        }
        ops
    }
    pub fn query_scalar(&self) -> (&Self, bool) {
        match self {
            Self::Optional { item } => (item, true),
            _ => (self, false),
        }
    }
}

impl Model {
    /// Validate before SQL construction. For scale 2, "1.2" normalizes to "1.20"; "1.234"
    /// fails instead of rounding. Equivalent encodings produce the same query/cursor identity.
    pub fn validate_query(&self, input: &QueryOptions) -> Result<ValidatedQuery, ModelError> {
        self.validate()?;
        if input.filters.values().map(BTreeMap::len).sum::<usize>() > 8 {
            return Err(ModelError::Query);
        }
        let mut normalized = input.clone();
        for (field, operators) in &mut normalized.filters {
            let kind = self
                .fields
                .get(field)
                .ok_or(ModelError::Query)?
                .output_type()?;
            if operators.is_empty() {
                return Err(ModelError::Query);
            }
            for (operator, value) in operators {
                if !kind.query_operators().contains(operator) {
                    return Err(ModelError::Query);
                }
                if *operator == Comparison::IsNull {
                    if !matches!(value, QueryValue::Bool(_)) {
                        return Err(ModelError::Query);
                    }
                } else {
                    if matches!(value, QueryValue::Text(s) if s.len() > 1024) {
                        return Err(ModelError::Query);
                    }
                    *value = normalize(kind.query_scalar().0, value)?;
                }
            }
        }
        if let Some(sort) = &normalized.sort {
            if !self.fields.contains_key(&sort.field) {
                return Err(ModelError::Query);
            }
        }
        let document = serde_json::json!(["cantongrid-model-query-v1", normalized]);
        let fingerprint = Sha256::digest(document.to_string().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(ValidatedQuery {
            options: normalized,
            fingerprint,
        })
    }

    /// Validate a non-null sort key again when resuming a signed/internal page cursor.
    pub fn validate_sort_value(
        &self,
        field: &str,
        value: &QueryValue,
    ) -> Result<QueryValue, ModelError> {
        let kind = self
            .fields
            .get(field)
            .ok_or(ModelError::Query)?
            .output_type()?;
        normalize(kind.query_scalar().0, value)
    }

    pub fn query_fields(&self) -> Result<Vec<QueryField>, ModelError> {
        self.validate()?;
        self.fields
            .iter()
            .map(|(name, expression)| {
                let kind = expression.output_type()?;
                let (scalar, nullable) = kind.query_scalar();
                let (label, scale) = match scalar {
                    FieldType::Text => ("text", None),
                    FieldType::Party => ("party", None),
                    FieldType::Bool => ("bool", None),
                    FieldType::Int64 => ("int64", None),
                    FieldType::Numeric { scale } => ("numeric", Some(*scale)),
                    FieldType::Optional { .. } => return Err(ModelError::Query),
                };
                Ok(QueryField {
                    name: name.clone(),
                    kind: label,
                    nullable,
                    scale,
                    operators: kind
                        .query_operators()
                        .into_iter()
                        .map(Comparison::name)
                        .collect(),
                })
            })
            .collect()
    }
}

/// Reuse the projection decoder's exact type/scale checks rather than accepting SQL's rounding.
fn normalize(kind: &FieldType, value: &QueryValue) -> Result<QueryValue, ModelError> {
    let json = serde_json::to_value(value).map_err(|_| ModelError::Query)?;
    match crate::evaluate::convert(kind, Some(&json)).map_err(|_| ModelError::Query)? {
        Value::Text(s) | Value::Party(s) | Value::Int64(s) => Ok(QueryValue::Text(s)),
        Value::Numeric(n) => Ok(QueryValue::Text(n.decimal)),
        Value::Bool(b) => Ok(QueryValue::Bool(b)),
        _ => Err(ModelError::Query),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;
    #[test]
    fn queries_are_exact_bounded_and_do_not_expose_definition_literals() {
        let model = Catalog::parse(include_str!("../assets/models/visibility.json"))
            .unwrap()
            .models()[0]
            .clone();
        let parse = |text: &str| serde_json::from_str::<QueryOptions>(text).unwrap();
        let a = model
            .validate_query(&parse(
                r#"{"filters":{"amount":{"gte":"1.2"}},"sort":{"field":"sequence"}}"#,
            ))
            .unwrap();
        let b=model.validate_query(&parse(r#"{"sort":{"field":"sequence","direction":"asc"},"filters":{"amount":{"gte":"1.2000000000"}}}"#)).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        for invalid in [
            r#"{"filters":{"missing":{"eq":"a"}}}"#,
            r#"{"filters":{"label":{"gt":"a"}}}"#,
            r#"{"filters":{"amount":{"eq":"1.00000000001"}}}"#,
            r#"{"filters":{"sequence":{"eq":"9223372036854775808"}}}"#,
            r#"{"filters":{"amount":{"isNull":true}}}"#,
            r#"{"sort":{"field":"amount;DROP TABLE x"}}"#,
            r#"{"filters":{"label":{}}}"#,
        ] {
            assert!(model.validate_query(&parse(invalid)).is_err());
        }
        assert!(
            serde_json::from_str::<QueryOptions>(r#"{"filters":{"amount":{"eq":1.2}}}"#).is_err()
        );
        assert!(
            serde_json::from_str::<QueryOptions>(r#"{"filters":{"note":{"eq":null}}}"#).is_err()
        );
        let fields = serde_json::to_value(model.query_fields().unwrap()).unwrap();
        assert!(fields.to_string().contains("isNull"));
        assert!(!fields.to_string().contains("path"));
    }
}
