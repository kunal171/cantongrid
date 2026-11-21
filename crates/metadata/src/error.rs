//! Stable errors intentionally omit archive paths, raw protobuf payloads and untrusted parser text.
use std::fmt;

/// Offline metadata errors distinguish unsupported versions from corruption and resource exhaustion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataError {
    Io,
    Limit,
    Archive,
    Manifest,
    Hash,
    UnsupportedVersion,
    Protobuf,
    Reference,
    Definition,
    UnsupportedType,
    Output,
}
impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "cannot read regular DAR file",
            Self::Limit => "DAR metadata resource limit exceeded",
            Self::Archive => "invalid or unsupported DAR container",
            Self::Manifest => "invalid or ambiguous DAR manifest",
            Self::Hash => "DALF payload does not match its declared SHA-256 identity",
            Self::UnsupportedVersion => {
                "unsupported Daml-LF version; this extractor accepts LF 2.1 only"
            }
            Self::Protobuf => "invalid metadata protobuf structure",
            Self::Reference => "invalid or unresolved metadata reference",
            Self::Definition => "invalid or ambiguous metadata declaration",
            Self::UnsupportedType => "unsupported LF metadata type or kind",
            Self::Output => "cannot encode bounded metadata output",
        })
    }
}
impl std::error::Error for MetadataError {}
