//! Deterministic DDL plans for owned model tables. Producing a plan executes no SQL.
use crate::{FieldType, Model, ModelError};
use serde::Serialize;

/// Exact compiler output shared by DDL generation and database inspection. These are not
/// deserialized plans: callers must compile them from a validated model.
#[derive(Clone, PartialEq, Eq)]
pub struct SqlColumn {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
}

impl std::fmt::Debug for SqlColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SqlColumn([redacted])")
    }
}

/// A reviewable generated schema proposal. Model revisions are immutable; applying/promoting
/// a plan and maintaining visibility/lifecycle are separate storage responsibilities.
#[derive(Serialize)]
pub struct SqlPlan {
    pub model: String,
    pub revision: u32,
    pub fingerprint: String,
    pub table: String,
    pub create_sql: String,
}
impl Model {
    /// Preserve column order, native type/scale and nullability for exact schema checks.
    pub fn sql_columns(&self) -> Result<Vec<SqlColumn>, ModelError> {
        self.validate()?;
        let mut columns = vec![
            SqlColumn {
                name: "_cg_stream_key".into(),
                sql_type: "text".into(),
                nullable: false,
            },
            SqlColumn {
                name: "_cg_contract_id".into(),
                sql_type: "text".into(),
                nullable: false,
            },
        ];
        for (name, expression) in &self.fields {
            let (kind, nullable) = match expression.output_type()? {
                FieldType::Optional { item } => (*item, true),
                kind => (kind, false),
            };
            let sql_type = match kind {
                FieldType::Text | FieldType::Party => "text".into(),
                FieldType::Bool => "boolean".into(),
                FieldType::Int64 => "bigint".into(),
                FieldType::Numeric { scale } => format!("numeric(38,{scale})"),
                FieldType::Optional { .. } => return Err(ModelError::Type),
            };
            columns.push(SqlColumn {
                name: name.clone(),
                sql_type,
                nullable,
            });
        }
        Ok(columns)
    }

    /// All identifiers are validated before quoting; table names derive from canonical hashes.
    /// DDL targets only CantonGrid-owned storage, never an upstream/source schema.
    pub fn sql_plan(&self) -> Result<SqlPlan, ModelError> {
        self.validate()?;
        let fingerprint = self.fingerprint()?;
        // 9 + 48 bytes stays below PostgreSQL's 63-byte identifier limit, avoiding truncation.
        let table = format!("cg_model_{}", &fingerprint[..48]);
        let mut columns: Vec<_> = self
            .sql_columns()?
            .into_iter()
            .map(|column| {
                format!(
                    "\"{}\" {}{}",
                    column.name,
                    column.sql_type,
                    if column.nullable { "" } else { " NOT NULL" }
                )
            })
            .collect();
        columns.push("PRIMARY KEY (\"_cg_stream_key\",\"_cg_contract_id\")".into());
        columns.push("FOREIGN KEY (\"_cg_stream_key\",\"_cg_contract_id\") REFERENCES cantongrid.contracts(stream_key,contract_id)".into());
        let create_sql = format!(
            "CREATE TABLE cantongrid_models.\"{table}\" (\n    {}\n);\nREVOKE ALL ON cantongrid_models.\"{table}\" FROM PUBLIC;\n",
            columns.join(",\n    ")
        );
        Ok(SqlPlan {
            model: self.name.clone(),
            revision: self.revision,
            fingerprint,
            table,
            create_sql,
        })
    }
}

impl Model {
    /// Encode validated projection values for PostgreSQL record insertion. Exact numbers stay
    /// strings until PostgreSQL's declared bigint/numeric column casts them, without float use.
    pub fn row_json(
        &self,
        stream: &str,
        contract: &str,
        fields: &std::collections::BTreeMap<String, cantongrid_core::value::Value>,
    ) -> Result<serde_json::Value, ModelError> {
        self.validate()?;
        if stream.is_empty()
            || contract.is_empty()
            || stream.len() > 8192
            || contract.len() > 8192
            || stream.chars().any(char::is_control)
            || contract.chars().any(char::is_control)
            || fields.len() != self.fields.len()
        {
            return Err(ModelError::Definition);
        }
        if self
            .filter
            .as_ref()
            .is_some_and(|filter| fields.get(&filter.field) != Some(&filter.equals))
        {
            return Err(ModelError::Type);
        }
        // Callers can provide typed rows directly, bypassing evaluate(). Keep its
        // projection-size bound here too, before cloning values into SQL JSON.
        if serde_json::to_vec(fields)
            .map_err(|_| ModelError::Type)?
            .len()
            > 1024 * 1024
        {
            return Err(ModelError::Limit);
        }
        let mut row = serde_json::Map::new();
        row.insert("_cg_stream_key".into(), stream.into());
        row.insert("_cg_contract_id".into(), contract.into());
        for (name, expression) in &self.fields {
            let value = fields.get(name).ok_or(ModelError::Type)?;
            expression.output_type()?.accepts(value)?;
            row.insert(name.clone(), plain(value)?);
        }
        Ok(row.into())
    }
}

/// Encode supported scalar SQL-row values without converting Int64/Numeric through JSON numbers.
fn plain(value: &cantongrid_core::value::Value) -> Result<serde_json::Value, ModelError> {
    use cantongrid_core::value::Value;
    Ok(match value {
        Value::Text(s) | Value::Party(s) | Value::Int64(s) => s.clone().into(),
        Value::Numeric(n) => n.decimal.clone().into(),
        Value::Bool(v) => (*v).into(),
        Value::Optional(None) => serde_json::Value::Null,
        Value::Optional(Some(v)) => return plain(v),
        _ => return Err(ModelError::Type),
    })
}
