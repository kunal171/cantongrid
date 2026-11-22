//! Only validated compiler identifiers enter SQL; endpoint identities and caller parties stay bound.
use cantongrid_models::FieldType;
use std::collections::BTreeMap;

/// Encode selected native columns without routing Int64/Numeric through JSON numbers.
pub(super) fn values(alias: &str, fields: &BTreeMap<String, FieldType>) -> String {
    let columns = fields
        .iter()
        .map(|(name, kind)| {
            let boolean = matches!(kind, FieldType::Bool)
                || matches!(kind,FieldType::Optional{item} if matches!(**item,FieldType::Bool));
            format!(
                "'{name}',{alias}.\"{name}\"{}",
                if boolean { "" } else { "::text" }
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("jsonb_build_object({columns})")
}

/// Same independent source endpoint predicate for initial visibility and the equality subquery.
pub(super) fn root(relation: &str) -> String {
    format!(
        "FROM {relation} s JOIN cantongrid.contracts l ON l.stream_key=s.\"_cg_stream_key\" AND l.contract_id=s.\"_cg_contract_id\" WHERE l.stream_key=$1 AND l.contract_id=$3 AND l.active AND l.witnesses && $2::text[] AND l.template=$4"
    )
}

/// Inner equality only. An absent/hidden target never yields an unmatched-root placeholder.
pub(super) fn targets(relation: &str, target_key: &str, source_key: &str, root: &str) -> String {
    format!(
        "FROM {relation} t JOIN cantongrid.contracts c ON c.stream_key=t.\"_cg_stream_key\" AND c.contract_id=t.\"_cg_contract_id\" WHERE c.stream_key=$1 AND c.active AND c.witnesses && $2::text[] AND c.template=$5 AND c.contract_id<>$3 AND t.\"{target_key}\"=(SELECT s.\"{source_key}\" {root})"
    )
}
