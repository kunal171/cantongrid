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
    /// Joined values require explicit opt-in; absence preserves M3 fingerprints and API behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<crate::JoinProjection>,
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
        if let Some(join) = &link.join {
            join.validate_source(model)?;
        }
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
        if let Some(join) = &link.join {
            join.validate_target(target)?;
        }
        Ok(())
    }

    /// A separate generated field domain keeps declared links distinct from row and aggregate roots.
    pub fn relationship_api_name(&self, name: &str) -> Result<String, ModelError> {
        self.relationship(name)?;
        Ok(format!("{}_relationship_{}", self.api_name()?, name))
    }
}

impl Relationship {
    /// Public provenance vocabulary shared by generated SDL and the runtime schema.
    pub const OUTPUT_FIELDS: &[&str] = &[
        "sourceContractId",
        "targetContractId",
        "relationship",
        "declaration",
        "declarationRevision",
        "sourceModelRevision",
        "targetModelRevision",
        "sourceFingerprint",
        "targetFingerprint",
        "coverage",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;
    const EXAMPLE: &str = include_str!("../assets/models/visibility.json");

    #[test]
    fn relationship_keys_reject_optional_boolean_scale_drift_and_untrusted_fields() {
        let model = Catalog::parse(EXAMPLE).unwrap().models()[0].clone();
        let link = Relationship {
            name: "links".into(),
            revision: 1,
            source_field: "amount".into(),
            target: RelationshipTarget {
                model: model.name.clone(),
                revision: 1,
                field: "amount".into(),
            },
            max_targets: 10,
            join: None,
        };
        let mut source = model.clone();
        source.relationships.push(link);
        source.validate_relationship("links", &model).unwrap();
        let mut target = model.clone();
        target.fields.insert(
            "amount".into(),
            crate::Expression::Field {
                path: vec!["amount".into()],
                value_type: FieldType::Numeric { scale: 2 },
            },
        );
        assert!(source.validate_relationship("links", &target).is_err());
        for kind in [
            FieldType::Bool,
            FieldType::Optional {
                item: Box::new(FieldType::Text),
            },
        ] {
            source.fields.insert(
                "key".into(),
                crate::Expression::Field {
                    path: vec!["key".into()],
                    value_type: kind,
                },
            );
            source.relationships[0].source_field = "key".into();
            assert!(source.validate().is_err());
        }
        source = model;
        source.relationships = vec![Relationship {
            name: "bad-name".into(),
            revision: 1,
            source_field: "label".into(),
            target: RelationshipTarget {
                model: source.name.clone(),
                revision: 1,
                field: "label".into(),
            },
            max_targets: 10,
            join: None,
        }];
        assert!(source.validate().is_err());
        let mut wire = serde_json::to_value(&source).unwrap();
        wire["relationships"][0]["name"] = "links".into();
        wire["relationships"][0]["sql"] = "SELECT 1".into();
        assert!(serde_json::from_value::<Model>(wire).is_err());
    }

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
            join: None,
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
