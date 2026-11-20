//! Closed aggregate declarations and trusted SQL fragments; inputs remain native exact values.
use crate::{FieldType, Model, ModelError, definition::identifier};
use cantongrid_core::value::{Decimal, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Group complete contribution sets. No expression, SQL or caller permission is accepted here.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregation {
    pub group_by: Vec<String>,
    pub measures: BTreeMap<String, Measure>,
}

/// Count contracts or sum one required exact field. Optional/null arithmetic is unsupported.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Measure {
    Count {},
    Sum { field: String },
}

/// SQL compiled from strict definitions, never accepted from serialized operator plans.
/// `values` is evaluated only after whole-group visibility; sums widen before bounded casting.
pub struct AggregatePlan {
    pub groups: String,
    pub columns: String,
    pub key: String,
    pub measures: String,
    pub values: String,
}

impl Aggregation {
    /// Reject unknown fields, ambiguous names, optional inputs and unbounded declaration sizes.
    pub(crate) fn validate(&self, types: &BTreeMap<String, FieldType>) -> Result<(), ModelError> {
        if !(1..=4).contains(&self.group_by.len()) || !(1..=8).contains(&self.measures.len()) {
            return Err(ModelError::Definition);
        }
        let mut seen = BTreeSet::new();
        for group in &self.group_by {
            if !seen.insert(group)
                || !types.contains_key(group)
                || matches!(types[group], FieldType::Optional { .. })
            {
                return Err(ModelError::Definition);
            }
        }
        for (name, measure) in &self.measures {
            if !identifier(name) || types.contains_key(name) {
                return Err(ModelError::Definition);
            }
            if let Measure::Sum { field } = measure {
                if !matches!(
                    types.get(field),
                    Some(FieldType::Int64 | FieldType::Numeric { .. })
                ) {
                    return Err(ModelError::Type);
                }
            }
        }
        Ok(())
    }
}

impl Model {
    /// Public aggregate fields include declared group keys and metrics, without processing literals.
    pub fn aggregate_fields(&self) -> Result<BTreeMap<String, FieldType>, ModelError> {
        self.validate()?;
        let aggregation = self.aggregation.as_ref().ok_or(ModelError::Definition)?;
        let mut fields = BTreeMap::new();
        for name in &aggregation.group_by {
            fields.insert(name.clone(), self.fields[name].output_type()?);
        }
        for (name, measure) in &aggregation.measures {
            let kind = match measure {
                Measure::Count {} => FieldType::Int64,
                Measure::Sum { field } => match self.fields[field].output_type()? {
                    // Two Int64-max inputs exceed Int64 but fit Numeric(38,0) exactly.
                    FieldType::Int64 => FieldType::Numeric { scale: 0 },
                    kind @ FieldType::Numeric { .. } => kind,
                    _ => return Err(ModelError::Type),
                },
            };
            fields.insert(name.clone(), kind);
        }
        Ok(fields)
    }

    /// Cursor domains/type names are disjoint from ordinary contract-row model queries.
    pub fn aggregate_api_name(&self) -> Result<String, ModelError> {
        self.aggregate_fields()?;
        Ok(format!(
            "aggregate_{}_{}",
            self.name,
            &self.fingerprint()?[..16]
        ))
    }

    /// Compile group and metric fragments; all aliases originate in validated identifiers.
    pub fn aggregate_plan(&self) -> Result<AggregatePlan, ModelError> {
        let fields = self.aggregate_fields()?;
        let aggregation = self.aggregation.as_ref().ok_or(ModelError::Definition)?;
        let mut groups = Vec::new();
        let mut columns = Vec::new();
        let mut keys = Vec::new();
        for name in &aggregation.group_by {
            let kind = self.fields[name].output_type()?;
            let mut column = format!("m.\"{name}\"");
            if matches!(kind, FieldType::Text | FieldType::Party) {
                column.push_str(" COLLATE \"C\"");
            }
            columns.push(format!("{column} AS \"{name}\""));
            keys.push(if kind == FieldType::Bool {
                column.clone()
            } else {
                format!("({column})::text")
            });
            groups.push(column);
        }
        let measures = aggregation
            .measures
            .iter()
            .map(|(name, measure)| match measure {
                Measure::Count {} => format!("count(*) AS \"{name}\""),
                Measure::Sum { field } => format!("sum(m.\"{field}\") AS \"{name}\""),
            })
            .collect::<Vec<_>>()
            .join(",");
        let values = fields
            .iter()
            .map(|(name, kind)| {
                let value = match (aggregation.measures.get(name), kind) {
                    (Some(Measure::Sum { .. }), FieldType::Numeric { scale }) => {
                        format!("g.\"{name}\"::numeric(38,{scale})::text")
                    }
                    (_, FieldType::Bool) => format!("g.\"{name}\""),
                    _ => format!("g.\"{name}\"::text"),
                };
                format!("'{name}',{value}")
            })
            .collect::<Vec<_>>()
            .join(",");
        Ok(AggregatePlan {
            groups: groups.join(","),
            columns: columns.join(","),
            key: format!("jsonb_build_array({})", keys.join(",")),
            measures,
            values,
        })
    }

    /// Decode only bounded typed keys; an ordinary row cursor cannot become an aggregate key.
    pub fn validate_aggregate_key(&self, key: &str) -> Result<(), ModelError> {
        self.validate()?;
        if key.len() > 4096 {
            return Err(ModelError::Limit);
        }
        let values: Vec<serde_json::Value> =
            cantongrid_core::document::parse(key).map_err(|_| ModelError::Definition)?;
        let aggregation = self.aggregation.as_ref().ok_or(ModelError::Definition)?;
        if values.len() != aggregation.group_by.len() {
            return Err(ModelError::Definition);
        }
        for (name, value) in aggregation.group_by.iter().zip(values) {
            let kind = self.fields[name].output_type()?;
            let value = match &kind {
                FieldType::Bool => Value::Bool(value.as_bool().ok_or(ModelError::Type)?),
                FieldType::Text => Value::Text(value.as_str().ok_or(ModelError::Type)?.into()),
                FieldType::Party => Value::Party(value.as_str().ok_or(ModelError::Type)?.into()),
                FieldType::Int64 => Value::Int64(value.as_str().ok_or(ModelError::Type)?.into()),
                FieldType::Numeric { scale } => Value::Numeric(Decimal {
                    scale: *scale,
                    decimal: value.as_str().ok_or(ModelError::Type)?.into(),
                }),
                _ => return Err(ModelError::Type),
            };
            kind.accepts(&value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;
    use serde_json::json;

    /// Reuse the canonical unrelated Daml example; changes affect only aggregate declarations.
    fn document() -> serde_json::Value {
        serde_json::from_str(include_str!("../assets/models/visibility.json")).unwrap()
    }
    fn aggregation() -> serde_json::Value {
        json!({"group_by":["state"],"measures":{"contracts":{"op":"count"},
            "total":{"op":"sum","field":"amount"},"units":{"op":"sum","field":"sequence"}}})
    }
    #[test]
    fn existing_fingerprints_and_exact_aggregate_types_are_preserved() {
        let mut doc = document();
        let old = Catalog::parse(&doc.to_string()).unwrap();
        assert_eq!(
            old.models()[0].fingerprint().unwrap(),
            "207804e002ff9b69208418ee9cf10e20d7fdc7aa7e282bf415f296344eb8f463"
        );
        doc["models"][0]["aggregation"] = aggregation();
        let catalog = Catalog::parse(&doc.to_string()).unwrap();
        let model = &catalog.models()[0];
        assert_ne!(
            model.fingerprint().unwrap(),
            old.models()[0].fingerprint().unwrap()
        );
        let fields = model.aggregate_fields().unwrap();
        assert_eq!(fields["units"], FieldType::Numeric { scale: 0 });
        assert_eq!(fields["total"], FieldType::Numeric { scale: 10 });
        assert_eq!(fields["contracts"], FieldType::Int64);
        let plan = model.aggregate_plan().unwrap();
        assert!(plan.groups.contains("COLLATE \"C\""));
        assert!(plan.values.contains("numeric(38,10)"));
        assert!(model.validate_aggregate_key(r#"["Ready"]"#).is_ok());
        for key in [
            r#"{"contract":"private"}"#,
            r#"[null]"#,
            r#"[]"#,
            r#"[true]"#,
        ] {
            assert!(model.validate_aggregate_key(key).is_err());
        }
    }
    #[test]
    fn unsafe_or_ambiguous_aggregate_definitions_are_rejected() {
        let cases = [
            json!({"group_by":[],"measures":{"n":{"op":"count"}}}),
            json!({"group_by":["state","state"],"measures":{"n":{"op":"count"}}}),
            json!({"group_by":["note"],"measures":{"n":{"op":"count"}}}),
            json!({"group_by":["missing"],"measures":{"n":{"op":"count"}}}),
            json!({"group_by":["state"],"measures":{"state":{"op":"count"}}}),
            json!({"group_by":["state"],"measures":{"items":{"op":"count"}}}),
            json!({"group_by":["state"],"measures":{"n":{"op":"sum","field":"note"}}}),
            json!({"group_by":["state"],"measures":{"n":{"op":"sum","field":"label"}}}),
            json!({"group_by":["state"],"measures":{"n":{"op":"average","field":"amount"}}}),
            json!({"group_by":["state"],"measures":{"n":{"op":"count","sql":"private"}}}),
        ];
        for input in cases {
            let mut doc = document();
            doc["models"][0]["aggregation"] = input;
            assert!(Catalog::parse(&doc.to_string()).is_err());
        }
    }
}
