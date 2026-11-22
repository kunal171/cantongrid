//! Declared relationship clients intentionally expose endpoint identities and policy provenance only.
use crate::{Model, ModelError, Relationship};

/// Generate exact public signatures from the same provenance vocabulary as the live schema.
pub(super) fn graphql(model: &Model, name: &str) -> Result<(String, String), ModelError> {
    let stem = format!("{}Relationship_{}", model.api_type()?, name);
    let mut text = format!("type {stem}Row {{\n");
    for field in Relationship::OUTPUT_FIELDS {
        text.push_str(&format!("  {field}: String!\n"));
    }
    text.push_str(&format!("  depth: Int!\n}}\ntype {stem}Page {{ items: [{stem}Row!]!, visibleTotal: String!, nextCursor: String, fingerprint: String! }}\n"));
    Ok((
        text,
        format!(
            "  {}(namespace: String!, contractId: String!, limit: Int, cursor: String): {stem}Page!\n",
            model.relationship_api_name(name)?
        ),
    ))
}

/// Financial keys are never returned or coerced to JS numbers; query roots and revisions are strings.
pub(super) fn typescript(model: &Model, name: &str) -> Result<String, ModelError> {
    let stem = format!("{}Relationship_{}", model.api_type()?, name);
    let mut text = format!("export interface {stem}Row {{\n");
    for field in Relationship::OUTPUT_FIELDS {
        let kind = match *field {
            "relationship" => "\"declared\"",
            "coverage" => "\"current_active_projection\"",
            _ => "string",
        };
        text.push_str(&format!("  readonly {field}: {kind};\n"));
    }
    text.push_str(&format!("  readonly depth: 1;\n}}\nexport interface {stem}Query {{ readonly namespace: string; readonly contractId: string; readonly limit?: number; readonly cursor?: string; }}\nexport interface {stem}Page {{ readonly items: readonly {stem}Row[]; readonly visibleTotal: string; readonly nextCursor: string | null; readonly fingerprint: string; }}\nexport const {} = {{ field: \"{}\", fingerprint: \"{}\" }} as const;\n",model.relationship_api_name(name)?,model.relationship_api_name(name)?,model.fingerprint()?));
    Ok(text)
}
