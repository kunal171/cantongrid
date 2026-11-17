//! Stable errors omit source payloads, literals, paths and credentials.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    Document,
    Definition,
    Type,
    Path,
    Missing,
    Overflow,
    Limit,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Document=>"invalid model document: check version, required fields, duplicates and unknown fields",
            Self::Definition=>"invalid model definition: require unique bounded names, positive revisions and exact templates",
            Self::Type=>"unsupported or mismatched model type, literal, arithmetic or filter",
            Self::Path=>"model paths require bounded object-field identifiers",
            Self::Missing=>"required model input is missing or null",
            Self::Overflow=>"model arithmetic exceeds the exact declared precision or integer range",
            Self::Limit=>"model document, expression or payload exceeds its supported bound",
        })
    }
}
impl std::error::Error for ModelError {}
