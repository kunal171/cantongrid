//! Internal source evidence, independent of contract transitions and public lifecycle APIs.
use crate::{
    config::{Plan, TemplateId, identifier},
    lifecycle::Batch,
    visibility::PartyId,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

/// Versioned participant-local receipt. JSON strings preserve exact source numeric encodings.
/// Witnesses describe this exercise; controllers and contract stakeholders cannot replace them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exercise {
    pub version: u32,
    pub offset: i64,
    pub node_id: i32,
    pub transaction_id: String,
    pub contract: String,
    pub template: TemplateId,
    pub choice: String,
    pub consuming: bool,
    pub argument: String,
    pub result: String,
    pub controllers: BTreeSet<String>,
    pub witnesses: BTreeSet<String>,
    pub ledger_at_micros: Option<i64>,
    /// -1 means unknown, never an inferred leaf or a proven subtree boundary.
    pub last_descendant_node_id: i32,
}

/// One atomic delivery. Snapshot pages carry no historical exercise receipts.
#[derive(Clone, PartialEq, Eq)]
pub struct ExercisePage {
    pub batch: Batch,
    pub exercises: Vec<Exercise>,
}

impl fmt::Debug for Exercise {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Exercise([redacted])")
    }
}

impl Exercise {
    /// Validate untrusted evidence without authenticating it or requiring a prior indexed create.
    pub fn validate(&self, plan: &Plan) -> bool {
        let text = |value: &str| identifier(value) && value.len() <= 1024;
        let parties = |values: &BTreeSet<String>| {
            !values.is_empty()
                && values.len() <= 256
                && values
                    .iter()
                    .all(|p| text(p) && PartyId::new(p.clone()).is_ok())
        };
        self.version == 1
            && plan.is_live()
            && self.offset > 0
            && self.node_id >= 0
            && (self.last_descendant_node_id == -1 || self.last_descendant_node_id >= self.node_id)
            && text(&self.transaction_id)
            && text(&self.contract)
            && self.choice.len() <= 256
            && !self.choice.is_empty()
            && self
                .choice
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && self.template.validate().is_ok()
            && plan.selects(&self.template)
            && parties(&self.controllers)
            && parties(&self.witnesses)
            && plan
                .source_parties()
                .any(|p| self.witnesses.contains(p.as_str()))
            && self.ledger_at_micros.is_none_or(|t| t >= 0)
            && [&self.argument, &self.result].into_iter().all(|value| {
                value.len() <= 65536
                    && serde_json::from_str::<Box<serde_json::value::RawValue>>(value).is_ok()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_payloads_unknown_bounds_and_independent_witnesses_are_preserved() {
        let plan = Plan::pqs(
            crate::config::SourceContext::new("source".into(), "test".into(), 1).unwrap(),
            [PartyId::new("A").unwrap()].into(),
            [TemplateId {
                package_id: "0".repeat(64),
                module: "M".into(),
                entity: "T".into(),
            }]
            .into(),
        )
        .unwrap();
        let mut receipt = Exercise {
            version: 1,
            offset: 9007199254740993,
            node_id: 2,
            transaction_id: "tx".into(),
            contract: "not-indexed".into(),
            template: plan.templates().next().unwrap().clone(),
            choice: "Create".into(),
            consuming: false,
            argument: r#"{"quantity":"9223372036854775807"}"#.into(),
            result: "null".into(),
            controllers: ["operator".into()].into(),
            witnesses: ["A".into()].into(),
            ledger_at_micros: None,
            last_descendant_node_id: -1,
        };
        assert!(receipt.validate(&plan));
        let decoded: Exercise =
            serde_json::from_str(&serde_json::to_string(&receipt).unwrap()).unwrap();
        assert_eq!(receipt, decoded);
        receipt.witnesses = ["operator".into()].into();
        assert!(!receipt.validate(&plan));
        receipt.witnesses = ["A".into()].into();
        receipt.last_descendant_node_id = 1;
        assert!(!receipt.validate(&plan));
        receipt.last_descendant_node_id = 2;
        receipt.version = 2;
        assert!(!receipt.validate(&plan));
        receipt.version = 1;
        receipt.argument = serde_json::to_string(&"x".repeat(65536)).unwrap();
        assert!(!receipt.validate(&plan));
        receipt.argument = "{}".into();
        receipt.result = "broken JSON".into();
        assert!(!receipt.validate(&plan));
        receipt.result = "null".into();
        receipt.controllers.clear();
        assert!(!receipt.validate(&plan));
    }
}
