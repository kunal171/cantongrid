//! Model-level join controls are deterministic and require no ledger or database.
use crate::{Catalog, JoinKind, JoinProjection};

#[test]
fn explicit_join_columns_are_typed_bounded_and_preserve_absent_fingerprints() {
    let text = include_str!("../assets/models/visibility.json");
    let base = Catalog::parse(text).unwrap().models()[0].clone();
    let mut source = base.clone();
    source.relationships.push(crate::Relationship {
        name: "related".into(),
        revision: 1,
        source_field: "label".into(),
        target: crate::RelationshipTarget {
            model: base.name.clone(),
            revision: 1,
            field: "label".into(),
        },
        max_targets: 100,
        join: None,
    });
    let old = source.fingerprint().unwrap();
    let mut wire = serde_json::to_value(&source).unwrap();
    assert!(wire["relationships"][0].get("join").is_none());
    wire["relationships"][0]["join"] = serde_json::Value::Null;
    assert_eq!(
        serde_json::from_value::<crate::Model>(wire.clone())
            .unwrap()
            .fingerprint()
            .unwrap(),
        old
    );
    let projection = JoinProjection {
        kind: JoinKind::Inner,
        source_fields: vec!["label".into(), "note".into()],
        target_fields: vec!["amount".into(), "sequence".into()],
    };
    source.relationships[0].join = Some(projection.clone());
    let (left, right) = source.join_fields("related", &base).unwrap();
    assert!(matches!(left["note"], crate::FieldType::Optional { .. }));
    assert!(matches!(
        right["amount"],
        crate::FieldType::Numeric { scale: 10 }
    ));
    assert_ne!(source.fingerprint().unwrap(), old);
    for names in [
        vec![],
        vec!["label".into(), "label".into()],
        vec!["missing".into()],
        vec!["label".into(); 17],
    ] {
        source.relationships[0].join.as_mut().unwrap().source_fields = names;
        assert!(source.validate().is_err());
    }
    source.relationships[0].join = Some(projection.clone());
    source.relationships[0].join.as_mut().unwrap().target_fields = vec!["missing".into()];
    assert!(source.join_fields("related", &base).is_err());
    wire["relationships"][0]["join"] =
        serde_json::json!({"kind":"left","source_fields":["label"],"target_fields":["label"]});
    assert!(serde_json::from_value::<crate::Model>(wire.clone()).is_err());
    wire["relationships"][0]["join"]["kind"] = "inner".into();
    wire["relationships"][0]["join"]["sql"] = "SELECT 1".into();
    assert!(serde_json::from_value::<crate::Model>(wire).is_err());
    source.relationships[0].join = Some(projection);
    let mut alias = base.clone();
    alias.fields["note"] = crate::Expression::Field {
        path: vec!["note".into()],
        value_type: crate::FieldType::Text,
    };
    assert_ne!(
        source.join_api_name("related", &base).unwrap(),
        source.join_api_name("related", &alias).unwrap()
    );
}
