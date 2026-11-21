//! Bounded offline DAR signatures, independent from source authorization, execution and migrations.
#![forbid(unsafe_code)]
mod container;
mod decode;
mod error;
mod manifest;
pub mod schema;
mod validate;
mod wire;
pub use error::MetadataError;
use schema::Metadata;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

/// Output is bounded during encoding, before a huge intermediate JSON string can be allocated.
const MAX_OUTPUT: usize = 32 * 1024 * 1024;
struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_OUTPUT.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("metadata limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Inspect only a bounded regular artifact. No network requests, uploads, package execution or DDL.
pub fn inspect_file(path: &Path) -> Result<Metadata, MetadataError> {
    if !std::fs::metadata(path)
        .map_err(|_| MetadataError::Io)?
        .is_file()
    {
        return Err(MetadataError::Io);
    }
    let file = std::fs::File::open(path).map_err(|_| MetadataError::Io)?;
    if !file.metadata().map_err(|_| MetadataError::Io)?.is_file() {
        return Err(MetadataError::Io);
    }
    let mut bytes = Vec::new();
    file.take(container::MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MetadataError::Io)?;
    inspect(&bytes)
}

/// Verify every original package hash and all extracted type links before publishing metadata.
pub fn inspect(bytes: &[u8]) -> Result<Metadata, MetadataError> {
    let archive = container::read(bytes)?;
    let mut packages = Vec::new();
    let mut ids = BTreeSet::new();
    let mut main = None;
    let mut budget = decode::Budget::new();
    for (path, bytes) in &archive.dalfs {
        let decoded = decode::decode(bytes)?;
        if !ids.insert(decoded.id.clone()) {
            return Err(MetadataError::Definition);
        }
        if path == &archive.manifest.main {
            main = Some(decoded.id.clone());
        }
        packages.push(decode::describe(&decoded, &mut budget)?);
    }
    packages.sort_by(|a, b| a.package_id.cmp(&b.package_id));
    validate::validate(&packages)?;
    let metadata = Metadata {
        format_version: 1,
        artifact_sha256: decode::hex(&Sha256::digest(bytes)),
        main_package_id: main.ok_or(MetadataError::Manifest)?,
        packages,
    };
    // Enforce the output bound even when a caller only asks for the structured result.
    metadata.to_json()?;
    Ok(metadata)
}
impl Metadata {
    /// Stable JSON: package/declaration ordering is canonical; field and parameter ordering is retained.
    pub fn to_json(&self) -> Result<String, MetadataError> {
        let mut output = BoundedOutput(Vec::new());
        serde_json::to_writer_pretty(&mut output, self).map_err(|_| MetadataError::Limit)?;
        String::from_utf8(output.0).map_err(|_| MetadataError::Output)
    }
}
