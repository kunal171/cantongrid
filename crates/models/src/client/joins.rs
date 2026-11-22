//! Joined clients expose only explicitly selected typed model columns and declared provenance.
use crate::{Model, ModelError, Relationship};

/// SDL uses both model fingerprints so namespace aliases cannot silently reinterpret target columns.
pub(super) fn graphql(
    model: &Model,
    name: &str,
    target: &Model,
) -> Result<(String, String), ModelError> {
    let stem = model.join_api_type(name, target)?;
    let (source_fields, target_fields) = model.join_fields(name, target)?;
    let mut text = String::new();
    for (suffix, fields) in [
        ("SourceValues", source_fields),
        ("TargetValues", target_fields),
    ] {
        text.push_str(&format!("type {stem}{suffix} {{\n"));
        for (field, kind) in fields {
            text.push_str(&format!("  {field}: {}\n", super::graphql_type(&kind)));
        }
        text.push_str("}\n");
    }
    text.push_str(&format!("type {stem}Row {{\n"));
    for field in Relationship::OUTPUT_FIELDS {
        text.push_str(&format!("  {field}: String!\n"));
    }
    text.push_str(&format!("  depth: Int!\n  joinKind: String!\n  sourceValues: {stem}SourceValues!\n  targetValues: {stem}TargetValues!\n}}\ntype {stem}Page {{ items: [{stem}Row!]!, visibleTotal: String!, nextCursor: String, fingerprint: String! }}\n"));
    Ok((
        text,
        format!(
            "  {}(namespace: String!, contractId: String!, limit: Int, cursor: String): {stem}Page!\n",
            model.join_api_name(name, target)?
        ),
    ))
}

/// Exact values remain strings; bool/null types follow the immutable projected column definitions.
pub(super) fn typescript(model: &Model, name: &str, target: &Model) -> Result<String, ModelError> {
    let stem = model.join_api_type(name, target)?;
    let (source_fields, target_fields) = model.join_fields(name, target)?;
    let mut text = String::new();
    for (suffix, fields) in [
        ("SourceValues", source_fields),
        ("TargetValues", target_fields),
    ] {
        text.push_str(&format!("export interface {stem}{suffix} {{\n"));
        for (field, kind) in fields {
            text.push_str(&format!(
                "  readonly {field}: {};\n",
                super::typescript_type(&kind)
            ));
        }
        text.push_str("}\n");
    }
    text.push_str(&format!("export interface {stem}Row {{\n"));
    for field in Relationship::OUTPUT_FIELDS {
        let kind = match *field {
            "relationship" => "\"declared\"",
            "coverage" => "\"current_active_projection\"",
            _ => "string",
        };
        text.push_str(&format!("  readonly {field}: {kind};\n"));
    }
    text.push_str(&format!("  readonly depth: 1;\n  readonly joinKind: \"inner\";\n  readonly sourceValues: {stem}SourceValues;\n  readonly targetValues: {stem}TargetValues;\n}}\nexport interface {stem}Query {{ readonly namespace: string; readonly contractId: string; readonly limit?: number; readonly cursor?: string; }}\nexport interface {stem}Page {{ readonly items: readonly {stem}Row[]; readonly visibleTotal: string; readonly nextCursor: string | null; readonly fingerprint: string; }}\nexport const {} = {{ field: \"{}\", fingerprint: \"{}\" }} as const;\n",model.join_api_name(name,target)?,model.join_api_name(name,target)?,model.fingerprint()?));
    Ok(text)
}
