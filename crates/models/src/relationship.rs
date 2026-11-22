//! Versioned application declarations are semantic policies, never source ancestry proofs.
use crate::{FieldType, Model, ModelError, definition::identifier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One outgoing equality policy. Keys are selected projected fields, not SQL or executable code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub name: String,
    pub revision: u32,
    pub source_field: String,
    pub target: RelationshipTarget,
    /// Visible fanout only; hidden matches must not change query success or public counts.
    pub max_targets: u32,
}

/// Exact target revision within the source binding's namespace and physical stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipTarget {
    pub model: String,
    pub revision: u32,
    pub field: String,
}

/// Check local declarations without requiring unrelated targets to be in this catalog upload.
pub(crate) fn validate(model: &Model) -> Result<(), ModelError> {
    if model.relationships.len() > 8 {
        return Err(ModelError::Limit);
    }
    let mut names = BTreeSet::new();
    for link in &model.relationships {
        if !identifier(&link.name)
            || !names.insert(&link.name)
            || link.revision == 0
            || !identifier(&link.target.model)
            || !identifier(&link.target.field)
            || link.target.revision == 0
            || !(1..=100).contains(&link.max_targets)
        {
            return Err(ModelError::Definition);
        }
        key_type(model, &link.source_field)?;
    }
    Ok(())
}

/// Keys retain exact native types: Numeric scale 2 never silently equals scale 3 or Text.
fn key_type(model: &Model, field: &str) -> Result<FieldType, ModelError> {
    let kind = model
        .fields
        .get(field)
        .ok_or(ModelError::Definition)?
        .output_type()?;
    if matches!(
        kind,
        FieldType::Text | FieldType::Party | FieldType::Int64 | FieldType::Numeric { .. }
    ) {
        Ok(kind)
    } else {
        Err(ModelError::Type)
    }
}

impl Model {
    /// Resolve a locally validated named declaration, never one supplied by an API caller.
    pub fn relationship(&self, name: &str) -> Result<&Relationship, ModelError> {
        self.validate()?;
        self.relationships
            .iter()
            .find(|r| r.name == name)
            .ok_or(ModelError::Definition)
    }

    /// Validate target identity and exact key types again when resolving active bindings.
    pub fn validate_relationship(&self, name: &str, target: &Model) -> Result<(), ModelError> {
        let link = self.relationship(name)?;
        target.validate()?;
        if target.name != link.target.model
            || target.revision != link.target.revision
            || key_type(self, &link.source_field)? != key_type(target, &link.target.field)?
        {
            return Err(ModelError::Type);
        }
        Ok(())
    }

    /// A separate generated field domain keeps declared links distinct from row and aggregate roots.
    pub fn relationship_api_name(&self, name: &str) -> Result<String, ModelError> {
        self.relationship(name)?;
        Ok(format!("{}_relationship_{}", self.api_name()?, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;
    const EXAMPLE: &str = include_str!("../assets/models/visibility.json");

    #[test]
    fn declarations_are_versioned_typed_and_never_change_legacy_fingerprints() {
        let model = Catalog::parse(EXAMPLE).unwrap().models()[0].clone();
        let old = model.fingerprint().unwrap();
        let mut wire = serde_json::to_value(&model).unwrap();
        assert!(wire.get("relationships").is_none());
        wire["relationships"] = serde_json::json!([]);
        let round: Model = serde_json::from_value(wire).unwrap();
        assert_eq!(round.fingerprint().unwrap(), old);
        let field = model
            .fields
            .iter()
            .find(|(_, e)| matches!(e.output_type(), Ok(FieldType::Text)))
            .unwrap()
            .0
            .clone();
        let link = Relationship {
            name: "successors".into(),
            revision: 1,
            source_field: field.clone(),
            target: RelationshipTarget {
                model: model.name.clone(),
                revision: model.revision,
                field,
            },
            max_targets: 100,
        };
        let mut source = model.clone();
        source.relationships.push(link.clone());
        source.validate_relationship("successors", &model).unwrap();
        assert_ne!(source.fingerprint().unwrap(), old);
        source.relationships[0].revision = 2;
        assert_ne!(source.fingerprint().unwrap(), {
            let mut first = source.clone();
            first.relationships[0].revision = 1;
            first.fingerprint().unwrap()
        });
        let mut target = model.clone();
        target.revision += 1;
        assert!(source.validate_relationship("successors", &target).is_err());
        source.relationships.push(link);
        assert!(source.validate().is_err());
        source.relationships.pop();
        source.relationships[0].source_field = "missing".into();
        assert!(source.validate().is_err());
        source.relationships[0].source_field = model.fields.keys().next().unwrap().clone();
        source.relationships[0].max_targets = 101;
        assert!(source.validate().is_err());
    }
}
