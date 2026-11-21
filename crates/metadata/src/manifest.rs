//! Strict DAR manifest parsing is independent of ZIP/protobuf decoding and executes no package code.
use crate::error::MetadataError;
use std::collections::{BTreeMap, BTreeSet};

/// Manifest input is capped before UTF-8 conversion or continuation-line joining.
pub(crate) const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub(crate) const MAX_PACKAGES: usize = 256;

/// A complete, unambiguous manifest selection. SDK labels are informational, not LF compatibility proof.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Manifest {
    pub main: String,
    pub dalfs: BTreeSet<String>,
}

/// Reject paths that could alias or escape on another platform even though nothing is extracted to disk.
pub(crate) fn safe_path(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 4096
        || name.contains(['\\', ':'])
        || name.chars().any(char::is_control)
        || name.starts_with('/')
    {
        return false;
    }
    let path = name.strip_suffix('/').unwrap_or(name);
    !path.is_empty()
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Join JAR continuation lines, case-fold keys and reject sections/duplicates before selecting DALFs.
pub(crate) fn parse(bytes: &[u8]) -> Result<Manifest, MetadataError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(MetadataError::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| MetadataError::Manifest)?;
    let mut logical: Vec<String> = Vec::new();
    let mut ended = false;
    for line in text.lines() {
        if line.is_empty() {
            ended = true;
            continue;
        }
        if ended {
            return Err(MetadataError::Manifest);
        }
        if let Some(rest) = line.strip_prefix(' ') {
            logical
                .last_mut()
                .ok_or(MetadataError::Manifest)?
                .push_str(rest);
        } else {
            logical.push(line.to_owned());
        }
    }
    let mut fields = BTreeMap::new();
    for line in logical {
        let (key, value) = line.split_once(": ").ok_or(MetadataError::Manifest)?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.chars().any(char::is_control)
            || fields
                .insert(key.to_ascii_lowercase(), value.to_owned())
                .is_some()
        {
            return Err(MetadataError::Manifest);
        }
    }
    if fields.get("manifest-version").map(String::as_str) != Some("1.0")
        || fields.get("format").map(String::as_str) != Some("daml-lf")
        || fields.get("encryption").map(String::as_str) != Some("non-encrypted")
    {
        return Err(MetadataError::Manifest);
    }
    let main = fields.remove("main-dalf").ok_or(MetadataError::Manifest)?;
    let list = fields.remove("dalfs").ok_or(MetadataError::Manifest)?;
    let mut dalfs = BTreeSet::new();
    for entry in list.split(',').map(str::trim) {
        if !safe_path(entry) || !entry.ends_with(".dalf") || !dalfs.insert(entry.to_owned()) {
            return Err(MetadataError::Manifest);
        }
        if dalfs.len() > MAX_PACKAGES {
            return Err(MetadataError::Limit);
        }
    }
    if !dalfs.contains(&main) {
        return Err(MetadataError::Manifest);
    }
    Ok(Manifest { main, dalfs })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str =
        "Manifest-Version: 1.0\r\nFormat: daml-lf\r\nEncryption: non-encrypted\r\n";

    /// Continuations append bytes without adding spaces; case-insensitive keys remain unique.
    #[test]
    fn folded_manifest_preserves_main_and_dependency_paths() {
        let text = format!(
            "{HEADER}Main-Dalf: project/main.dalf\r\nDalfs: project/main.dalf,\r\n project/dependency.dalf\r\n\r\n"
        );
        let manifest = parse(text.as_bytes()).unwrap();
        assert_eq!(manifest.main, "project/main.dalf");
        assert_eq!(manifest.dalfs.len(), 2);
        assert!(manifest.dalfs.contains("project/dependency.dalf"));
        assert!(parse(format!("{text}Name: section\n").as_bytes()).is_err());
    }

    /// Ambiguous identities, duplicate packages, malformed encryption and absent main packages fail.
    #[test]
    fn malformed_manifests_never_choose_an_arbitrary_package() {
        for tail in [
            "Main-Dalf: a.dalf\nmain-dalf: b.dalf\nDalfs: a.dalf,b.dalf\n",
            "Main-Dalf: a.dalf\nDalfs: a.dalf,a.dalf\n",
            "Main-Dalf: a.dalf\nDalfs: b.dalf\n",
            "Main-Dalf: ../a.dalf\nDalfs: ../a.dalf\n",
            "Main-Dalf: a.dalf\nDalfs: a.dalf,\n",
            "Dalfs: a.dalf\n",
        ] {
            assert!(parse(format!("{HEADER}{tail}").as_bytes()).is_err());
        }
        assert!(parse(b" continuation without a preceding line").is_err());
        assert!(parse(&[0xff]).is_err());
        assert_eq!(
            parse(&vec![b'a'; MAX_MANIFEST_BYTES + 1]),
            Err(MetadataError::Limit)
        );
    }

    /// Portable archive paths must not depend on filesystem normalization or path traversal.
    #[test]
    fn unsafe_and_ambiguous_paths_are_rejected() {
        for name in [
            "",
            "/main.dalf",
            "../main.dalf",
            "a/../main.dalf",
            "a//b.dalf",
            "./a.dalf",
            "C:/a.dalf",
            "a\\b.dalf",
            "a\0b.dalf",
            "/",
        ] {
            assert!(!safe_path(name));
        }
        assert!(safe_path("package/main.dalf"));
        assert!(safe_path("META-INF/"));
    }
}
