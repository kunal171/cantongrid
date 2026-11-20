//! Aggregate client shapes are generated from the same validated output types as SQL.
use crate::{Model, ModelError};

/// Return extra GraphQL output definitions and its query root; inputs cannot filter contributors.
pub(super) fn graphql(model: &Model) -> Result<(String, String), ModelError> {
    let stem = format!("{}Aggregate", model.api_type()?);
    let mut types = format!("type {stem}Values {{\n");
    for (name, kind) in model.aggregate_fields()? {
        types.push_str(&format!("  {name}: {}\n", super::graphql_type(&kind)));
    }
    types.push_str(&format!("}}\ntype {stem}Row {{ groupKey: String!, values: {stem}Values! }}\ntype {stem}Page {{ items: [{stem}Row!]!, visibleTotal: String!, nextCursor: String, fingerprint: String! }}\n"));
    Ok((
        types,
        format!(
            "  {}(namespace: String!, limit: Int, cursor: String): {stem}Page!\n",
            model.aggregate_api_name()?
        ),
    ))
}

/// Exact sums/counts remain strings. groupKey is an opaque tie-breaker, never an authorization grant.
pub(super) fn typescript(model: &Model) -> Result<String, ModelError> {
    let stem = format!("{}Aggregate", model.api_type()?);
    let mut text = format!("export interface {stem}Values {{\n");
    for (name, kind) in model.aggregate_fields()? {
        text.push_str(&format!(
            "  readonly {name}: {};\n",
            super::typescript_type(&kind)
        ));
    }
    text.push_str(&format!("}}\nexport interface {stem}Query {{ readonly namespace: string; readonly limit?: number; readonly cursor?: string; }}\nexport interface {stem}Row {{ readonly groupKey: string; readonly values: {stem}Values; }}\nexport interface {stem}Page {{ readonly items: readonly {stem}Row[]; readonly visibleTotal: string; readonly nextCursor: string | null; readonly fingerprint: string; }}\nexport const {} = {{ field: \"{}\", fingerprint: \"{}\" }} as const;\n",model.aggregate_api_name()?,model.aggregate_api_name()?,model.fingerprint()?));
    Ok(text)
}
