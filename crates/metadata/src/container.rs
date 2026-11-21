//! Read bounded archive entries in memory; never extract files or execute package contents.
use crate::{error::MetadataError, manifest};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};
use zip::{CompressionMethod, read::Config, unstable::read::ZipIterable};

/// Bounds apply before ZIP table allocation, decompression and subsequent protobuf decoding.
pub(crate) const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 1024;
const MAX_ENTRY: u64 = 8 * 1024 * 1024;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;

/// Original DALF bytes retain their payload hashes and remain independent from untrusted path labels.
pub(crate) struct Container {
    pub manifest: manifest::Manifest,
    pub dalfs: BTreeMap<String, Vec<u8>>,
}

/// The exact pinned ZIP iterator checks count before collecting any per-entry table.
/// Header consistency and our duplicate/path checks prevent archive-view ambiguity.
pub(crate) fn read(bytes: &[u8]) -> Result<Container, MetadataError> {
    if bytes.len() > MAX_INPUT {
        return Err(MetadataError::Limit);
    }
    let mut archive = ZipIterable::try_new(Cursor::new(bytes), Config::default())
        .map_err(|_| MetadataError::Archive)?
        .enforce_header_consistency(true);
    if archive.len() > MAX_ENTRIES {
        return Err(MetadataError::Limit);
    }
    let mut names = BTreeSet::new();
    let mut dalfs = BTreeMap::new();
    let mut manifest_bytes = None;
    let mut total = 0u64;
    for entry in archive.files().map_err(|_| MetadataError::Archive)? {
        let entry = entry.map_err(|_| MetadataError::Archive)?;
        let name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| MetadataError::Archive)?
            .to_owned();
        if !manifest::safe_path(&name)
            || !names.insert(name.trim_end_matches('/').to_owned())
            || entry.encrypted()
            || entry.is_symlink()
            || !matches!(
                entry.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err(MetadataError::Archive);
        }
        if entry.is_dir() {
            if entry.size() != 0 {
                return Err(MetadataError::Archive);
            }
            continue;
        }
        if !entry.is_file() {
            return Err(MetadataError::Archive);
        }
        let cap = if name == "META-INF/MANIFEST.MF" {
            manifest::MAX_MANIFEST_BYTES as u64
        } else {
            MAX_ENTRY
        };
        if entry.size() > cap {
            return Err(MetadataError::Limit);
        }
        total = total
            .checked_add(entry.size())
            .ok_or(MetadataError::Limit)?;
        if total > MAX_TOTAL {
            return Err(MetadataError::Limit);
        }
        let expected = entry.size();
        let mut body = Cursor::new(bytes);
        let reader = entry
            .with_reader(&mut body, Default::default())
            .map_err(|_| MetadataError::Archive)?;
        let mut expanded = Vec::new();
        reader
            .take(cap + 1)
            .read_to_end(&mut expanded)
            .map_err(|_| MetadataError::Archive)?;
        if expanded.len() as u64 > cap {
            return Err(MetadataError::Limit);
        }
        if expanded.len() as u64 != expected {
            return Err(MetadataError::Archive);
        }
        if name == "META-INF/MANIFEST.MF" {
            manifest_bytes = Some(expanded);
        } else if name.ends_with(".dalf") {
            if dalfs.len() >= manifest::MAX_PACKAGES {
                return Err(MetadataError::Limit);
            }
            dalfs.insert(name, expanded);
        }
    }
    let manifest = manifest::parse(&manifest_bytes.ok_or(MetadataError::Manifest)?)?;
    if manifest.dalfs != dalfs.keys().cloned().collect() {
        return Err(MetadataError::Manifest);
    }
    Ok(Container { manifest, dalfs })
}
