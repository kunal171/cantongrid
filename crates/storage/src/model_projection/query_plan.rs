//! Compile only validated model identifiers and closed operators; values always remain parameters.
use crate::StoreError;
use cantongrid_models::{Comparison, Direction, Model, QueryValue, ValidatedQuery};
use serde::{Deserialize, Serialize};

/// Internal keyset, wrapped by the API's authenticated principal/source-bound cursor.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    query: String,
    contract: String,
    value: Option<QueryValue>,
}

/// Parameters start after stream, parties and template. Native SQL types preserve exact ordering.
pub(super) struct QueryPlan {
    pub filters: String,
    pub page: String,
    pub order: String,
    pub parameters: Vec<String>,
    columns: std::collections::BTreeMap<String, String>,
    query: ValidatedQuery,
}
impl QueryPlan {
    pub fn new(
        model: &Model,
        query: ValidatedQuery,
        after: Option<&str>,
    ) -> Result<Self, StoreError> {
        let columns = model
            .sql_columns()
            .map_err(|_| StoreError::ModelCorrupt)?
            .into_iter()
            .skip(2)
            .map(|c| (c.name, c.sql_type))
            .collect();
        let mut result = Self {
            filters: String::new(),
            page: String::new(),
            order: "c.contract_id ASC".into(),
            parameters: vec![],
            columns,
            query,
        };
        // Clone the small validated options so parameter allocation can mutate the plan.
        let options = result.query.options().clone();
        for (field, operators) in &options.filters {
            for (operator, value) in operators {
                let column = format!("m.\"{field}\"");
                let condition = if *operator == Comparison::IsNull {
                    match value {
                        QueryValue::Bool(true) => format!("{column} IS NULL"),
                        QueryValue::Bool(false) => format!("{column} IS NOT NULL"),
                        _ => return Err(StoreError::ModelQuery),
                    }
                } else {
                    let parameter = result.bind(value.parameter(), &result.columns[field].clone());
                    let op = match operator {
                        Comparison::Eq => "=",
                        Comparison::Ne => "<>",
                        Comparison::Gt => ">",
                        Comparison::Gte => ">=",
                        Comparison::Lt => "<",
                        Comparison::Lte => "<=",
                        Comparison::IsNull => unreachable!(),
                    };
                    format!("{column} {op} {parameter}")
                };
                result.filters.push_str(&format!(" AND ({condition})"));
            }
        }
        if let Some(sort) = &options.sort {
            let direction = if sort.direction == Direction::Asc {
                "ASC"
            } else {
                "DESC"
            };
            result.order = format!(
                "m.\"{}\" {direction} NULLS LAST,c.contract_id ASC",
                sort.field
            );
        }
        if let Some(after) = after {
            if after.len() > 4096 {
                return Err(StoreError::ModelQuery);
            }
            let key: Key = serde_json::from_str(after).map_err(|_| StoreError::ModelQuery)?;
            if key.query != result.query.fingerprint()
                || key.contract.is_empty()
                || key.contract.len() > 2048
                || key.contract.chars().any(char::is_control)
            {
                return Err(StoreError::ModelQuery);
            }
            let id = result.bind(key.contract, "text");
            result.page = if let Some(sort) = &options.sort {
                let column = format!("m.\"{}\"", sort.field);
                match key.value {
                    Some(value) => {
                        let value = model
                            .validate_sort_value(&sort.field, &value)
                            .map_err(|_| StoreError::ModelQuery)?;
                        let parameter =
                            result.bind(value.parameter(), &result.columns[&sort.field].clone());
                        let op = if sort.direction == Direction::Asc {
                            ">"
                        } else {
                            "<"
                        };
                        // NULLS LAST applies in both directions; equal values use ascending IDs.
                        format!(
                            " AND ({column} IS NULL OR {column}{op}{parameter} OR ({column}={parameter} AND c.contract_id>{id}))"
                        )
                    }
                    None => format!(" AND ({column} IS NULL AND c.contract_id>{id})"),
                }
            } else {
                if key.value.is_some() {
                    return Err(StoreError::ModelQuery);
                }
                format!(" AND c.contract_id>{id}")
            };
        }
        Ok(result)
    }
    /// All values are text parameters, explicitly cast only to compiler-owned native types.
    fn bind(&mut self, value: String, sql_type: &str) -> String {
        self.parameters.push(value);
        format!("(${}::text)::{sql_type}", self.parameters.len() + 3)
    }
    /// Store the last returned row's sort value, even when a client did not select that field.
    pub fn key(&self, row: &serde_json::Value) -> Result<String, StoreError> {
        let contract = row["contractId"]
            .as_str()
            .ok_or(StoreError::ModelCorrupt)?
            .to_owned();
        let value = self
            .query
            .options()
            .sort
            .as_ref()
            .map(|sort| {
                let value = &row["values"][&sort.field];
                if value.is_null() {
                    Ok(None)
                } else {
                    serde_json::from_value(value.clone())
                        .map(Some)
                        .map_err(|_| StoreError::ModelCorrupt)
                }
            })
            .transpose()?
            .flatten();
        let key = serde_json::to_string(&Key {
            query: self.query.fingerprint().into(),
            contract,
            value,
        })
        .map_err(|_| StoreError::ModelCorrupt)?;
        if key.len() > 4096 {
            return Err(StoreError::Limit);
        }
        Ok(key)
    }
}
