//! Package identities are resolved by explicit artifact membership, never semantic-version guesses.
use crate::SyncError as Error;
use cantongrid_metadata::schema::{Definition, Metadata, Package};
use serde::Serialize;
use std::collections::BTreeMap;

/// One whole-package signature change, including fields the application did not select.
#[derive(Serialize)]
pub struct Change {
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub package: String,
    pub declaration: String,
    pub kind: &'static str,
}
/// Prefer a matching new main package; otherwise require exactly one package with the old name.
/// Retaining an older dependency in a new DAR is not permission to pick a version silently.
pub(crate) fn replacement<'a>(old: &Package, new: &'a Metadata) -> Result<&'a Package, Error> {
    let candidates: Vec<_> = new.packages.iter().filter(|p| p.name == old.name).collect();
    if let Some(main) = candidates
        .iter()
        .find(|p| p.package_id == new.main_package_id)
    {
        return Ok(main);
    }
    if candidates.len() != 1 {
        return Err(Error::Identity);
    }
    Ok(candidates[0])
}
/// Normalize only qualified package-id fields. Business literals and arbitrary strings are never rewritten.
pub(crate) fn normalize(value: &mut serde_json::Value, mappings: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if key == "package_id" {
                    if let Some(replacement) = value.as_str().and_then(|id| mappings.get(id)) {
                        *value = replacement.clone().into();
                    }
                } else {
                    normalize(value, mappings);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for value in items {
                normalize(value, mappings);
            }
        }
        _ => {}
    }
}
/// Named map entries make additions/removals readable rather than comparing positional AST indexes.
fn declarations(package: &Package) -> Result<BTreeMap<String, serde_json::Value>, Error> {
    let mut result = BTreeMap::new();
    for declaration in &package.data_types {
        let stem = format!("{}:{}", declaration.id.module, declaration.id.name);
        result.insert(
            format!("type/{stem}/parameters"),
            serde_json::to_value((&declaration.parameters, declaration.serializable))
                .map_err(|_| Error::Definition)?,
        );
        match &declaration.definition {
            Definition::Record { fields }
            | Definition::Variant {
                constructors: fields,
            } => {
                result.insert(
                    format!("type/{stem}/shape"),
                    serde_json::to_value((
                        matches!(declaration.definition, Definition::Record { .. }),
                        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
                    ))
                    .map_err(|_| Error::Definition)?,
                );
                for field in fields {
                    result.insert(
                        format!("type/{stem}/field/{}", field.name),
                        serde_json::to_value(&field.value_type).map_err(|_| Error::Definition)?,
                    );
                }
            }
            other => {
                result.insert(
                    format!("type/{stem}/definition"),
                    serde_json::to_value(other).map_err(|_| Error::Definition)?,
                );
            }
        }
    }
    for alias in &package.synonyms {
        result.insert(
            format!("alias/{}:{}", alias.id.module, alias.id.name),
            serde_json::to_value(alias).map_err(|_| Error::Definition)?,
        );
    }
    for template in &package.templates {
        result.insert(
            format!("template/{}:{}", template.id.module, template.id.name),
            serde_json::to_value(template).map_err(|_| Error::Definition)?,
        );
    }
    for interface in &package.interfaces {
        result.insert(
            format!("interface/{}:{}", interface.id.module, interface.id.name),
            serde_json::to_value(interface).map_err(|_| Error::Definition)?,
        );
    }
    Ok(result)
}
/// Compare complete declarations for the selected packages, retaining a bounded, sorted change list.
pub(crate) fn changes(
    old: &Package,
    new: &Package,
    mappings: &BTreeMap<String, String>,
) -> Result<Vec<Change>, Error> {
    let before = declarations(old)?;
    let after = declarations(new)?;
    let names: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let mut result = Vec::new();
    for name in names {
        let comparable = before.get(name).cloned().map(|mut value| {
            normalize(&mut value, mappings);
            value
        });
        let kind = match (comparable.as_ref(), after.get(name)) {
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            (Some(a), Some(b)) if a != b => "changed",
            _ => continue,
        };
        if result.len() >= 4096 {
            return Err(Error::Limit);
        }
        result.push(Change {
            before: before.get(name).cloned(),
            after: after.get(name).cloned(),
            package: old.name.clone(),
            declaration: name.clone(),
            kind,
        });
    }
    Ok(result)
}
