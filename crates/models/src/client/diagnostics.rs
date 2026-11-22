//! Generated inspector interfaces contain only sanitized findings and public model identities.
use crate::{Model, ModelDiagnostic, ModelError};
/// Share exact field names with runtime schema construction.
pub(super) fn graphql() -> String {
    let mut result = String::from("type CgProjectionDiagnostic {\n");
    for field in ModelDiagnostic::OUTPUT_FIELDS {
        result.push_str(&format!("  {field}: String!\n"));
    }
    result.push_str("  active: Boolean!\n  errorCode: String\n}\ntype CgProjectionPage { items: [CgProjectionDiagnostic!]!, visibleTotal: String!, nextCursor: String, fingerprint: String! }\n");
    result
}
/// The operation diagnoses one current indexed input; clients cannot provide substitute payloads.
pub(super) fn root(model: &Model) -> Result<String, ModelError> {
    Ok(format!(
        "  {}(namespace: String!, contractId: String!): CgProjectionPage!\n",
        model.diagnostic_api_name()?
    ))
}
/// No source values, private checkpoint keys or parser error messages are part of the client shape.
pub(super) fn typescript() -> String {
    let mut result = String::from("export interface CgProjectionDiagnostic {\n");
    for field in ModelDiagnostic::OUTPUT_FIELDS {
        result.push_str(&format!("  readonly {field}: string;\n"));
    }
    result.push_str("  readonly active: boolean;\n  readonly errorCode: string | null;\n}\nexport interface CgProjectionQuery { readonly namespace: string; readonly contractId: string; }\nexport interface CgProjectionPage { readonly items: readonly CgProjectionDiagnostic[]; readonly visibleTotal: string; readonly nextCursor: null; readonly fingerprint: string; }\n");
    result
}
