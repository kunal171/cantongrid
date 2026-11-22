//! Conservative source proofs. Semantic field references and nearest-visible ancestors are not edges.
use crate::{
    config::{Plan, TemplateId, identifier},
    exercise::ExercisePage,
    visibility::PartyId,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Metadata only: observing a transient/divulged creation never adds a contract-state row.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creation {
    pub version: u32,
    pub offset: i64,
    pub node_id: i32,
    pub transaction_id: String,
    pub contract: String,
    pub template: TemplateId,
    pub witnesses: BTreeSet<String>,
    pub divulged_only: bool,
}

/// One supported transaction or an evidence-free partial-history snapshot page.
#[derive(Clone, PartialEq, Eq)]
pub struct LineagePage {
    pub delivery: ExercisePage,
    pub creations: Vec<Creation>,
}

/// Direct exercise→creation relationship, including every source witness obligation used in proof.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectEdge {
    pub version: u32,
    pub offset: i64,
    pub parent_node: i32,
    pub child_node: i32,
    pub transaction_id: String,
    pub parent_contract: String,
    pub child_contract: String,
    pub parent_template: TemplateId,
    pub child_template: TemplateId,
    pub choice: String,
    /// AND across nodes; any authorized witness within a node is sufficient.
    pub visibility_clauses: BTreeSet<BTreeSet<String>>,
}

impl fmt::Debug for Creation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Creation([redacted])")
    }
}
impl fmt::Debug for DirectEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DirectEdge([redacted])")
    }
}

impl Creation {
    /// Validate source identity and evidence without claiming this contract is caller-visible.
    pub fn validate(&self, plan: &Plan) -> bool {
        let text = |v: &str| identifier(v) && v.len() <= 1024;
        self.version == 1
            && self.offset > 0
            && self.node_id >= 0
            && text(&self.transaction_id)
            && text(&self.contract)
            && self.template.validate().is_ok()
            && plan.selects(&self.template)
            && !self.witnesses.is_empty()
            && self.witnesses.len() <= 256
            && self
                .witnesses
                .iter()
                .all(|p| text(p) && PartyId::new(p.clone()).is_ok())
            && plan
                .source_parties()
                .any(|p| self.witnesses.contains(p.as_str()))
    }
}

/// Reject contradictory source evidence; absence/unknown bounds simply yield no proven edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceError;
impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or oversized lineage evidence")
    }
}
impl std::error::Error for EvidenceError {}

/// Prove direct links only from complete, laminar source intervals. At most 64 nodes
/// means checking every candidate interval costs at most 64² comparisons, never an unbounded walk.
pub fn prove(plan: &Plan, page: &LineagePage) -> Result<Vec<DirectEdge>, EvidenceError> {
    prove_evidence(
        plan,
        &page.delivery.batch,
        &page.delivery.exercises,
        &page.creations,
    )
}

/// Borrow already bounded deliveries so storage validation does not copy financial payloads.
pub fn prove_evidence(
    plan: &Plan,
    batch: &crate::lifecycle::Batch,
    exercises: &[crate::exercise::Exercise],
    creations: &[Creation],
) -> Result<Vec<DirectEdge>, EvidenceError> {
    if !plan.includes_lineage()
        || batch.context != *plan.context()
        || exercises.len() + creations.len() > 64
    {
        return Err(EvidenceError);
    }
    let mut nodes = BTreeMap::new();
    let mut offsets = BTreeSet::new();
    let mut transactions = BTreeSet::new();
    let mut created_contracts = BTreeSet::new();
    for e in exercises {
        if !e.validate(plan) || nodes.insert(e.node_id, &e.witnesses).is_some() {
            return Err(EvidenceError);
        }
        offsets.insert(e.offset);
        transactions.insert(&e.transaction_id);
    }
    for c in creations {
        if !c.validate(plan)
            || nodes.insert(c.node_id, &c.witnesses).is_some()
            || !created_contracts.insert(&c.contract)
        {
            return Err(EvidenceError);
        }
        offsets.insert(c.offset);
        transactions.insert(&c.transaction_id);
    }
    if offsets.len() > 1 || transactions.len() > 1 {
        return Err(EvidenceError);
    }
    for event in &batch.events {
        let origin = event.origin.as_ref().ok_or(EvidenceError)?;
        if (origin.snapshot_observation && !nodes.is_empty())
            || (!origin.snapshot_observation
                && offsets.iter().any(|&offset| offset != origin.offset))
        {
            return Err(EvidenceError);
        }
    }
    // Crossing bounds cannot describe a tree, even if all numeric IDs happen to exist.
    for a in exercises {
        for b in exercises {
            if a.node_id < b.node_id
                && b.node_id <= a.last_descendant_node_id
                && b.last_descendant_node_id > a.last_descendant_node_id
            {
                return Err(EvidenceError);
            }
        }
    }
    let complete = |parent: &crate::exercise::Exercise| {
        let width = i64::from(parent.last_descendant_node_id) - i64::from(parent.node_id) + 1;
        width > 0
            && width <= 64
            && nodes
                .range(parent.node_id..=parent.last_descendant_node_id)
                .count()
                == width as usize
            && exercises
                .iter()
                .filter(|e| {
                    e.node_id >= parent.node_id && e.node_id <= parent.last_descendant_node_id
                })
                .all(|e| e.last_descendant_node_id >= e.node_id)
    };
    let mut result = vec![];
    let mut proof_bytes = 0_usize;
    for c in creations {
        let parent = exercises
            .iter()
            .filter(|e| e.node_id < c.node_id && c.node_id <= e.last_descendant_node_id)
            .max_by_key(|e| e.node_id);
        let Some(parent) = parent.filter(|e| complete(e)) else {
            continue;
        };
        if parent.contract == c.contract {
            return Err(EvidenceError);
        }
        // A known containing interval does not prove directness around an unknown nested bound.
        if exercises.iter().any(|e| {
            e.node_id > parent.node_id && e.node_id < c.node_id && e.last_descendant_node_id < 0
        }) {
            continue;
        }
        let clauses: BTreeSet<_> = nodes
            .range(parent.node_id..=parent.last_descendant_node_id)
            .map(|(_, w)| *w)
            .collect();
        proof_bytes = proof_bytes
            .checked_add(
                serde_json::to_vec(&clauses)
                    .map_err(|_| EvidenceError)?
                    .len(),
            )
            .ok_or(EvidenceError)?;
        if proof_bytes > 1024 * 1024 {
            return Err(EvidenceError);
        }
        result.push(DirectEdge {
            version: 1,
            offset: c.offset,
            parent_node: parent.node_id,
            child_node: c.node_id,
            transaction_id: c.transaction_id.clone(),
            parent_contract: parent.contract.clone(),
            child_contract: c.contract.clone(),
            parent_template: parent.template.clone(),
            child_template: c.template.clone(),
            choice: parent.choice.clone(),
            visibility_clauses: clauses.into_iter().cloned().collect(),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::SourceContext, exercise::Exercise, lifecycle::Batch};
    /// A seven-node nested transaction with a transient child, matching the generic Daml fixture.
    fn fixture() -> (Plan, LineagePage) {
        let template = TemplateId {
            package_id: "0".repeat(64),
            module: "M".into(),
            entity: "T".into(),
        };
        let plan = Plan::pqs(
            SourceContext::new("lineage".into(), "synthetic".into(), 1).unwrap(),
            [PartyId::new("A").unwrap(), PartyId::new("B").unwrap()].into(),
            [template.clone()].into(),
        )
        .unwrap()
        .with_exercises()
        .unwrap()
        .with_lineage()
        .unwrap();
        let exercises = [(0, 6, "factory"), (2, 5, "relay"), (4, 4, "temporary")]
            .into_iter()
            .map(|(node, last, contract)| Exercise {
                version: 1,
                offset: 9007199254740993,
                node_id: node,
                transaction_id: "tx".into(),
                contract: contract.into(),
                template: template.clone(),
                choice: "Create".into(),
                consuming: true,
                argument: "{}".into(),
                result: "null".into(),
                controllers: ["operator".into()].into(),
                witnesses: ["A".into(), "B".into()].into(),
                ledger_at_micros: None,
                last_descendant_node_id: last,
            })
            .collect();
        let creations = [
            (1, "relay"),
            (3, "temporary"),
            (5, "permanent"),
            (6, "successor"),
        ]
        .into_iter()
        .map(|(node, contract)| Creation {
            version: 1,
            offset: 9007199254740993,
            node_id: node,
            transaction_id: "tx".into(),
            contract: contract.into(),
            template: template.clone(),
            witnesses: ["A".into(), "B".into()].into(),
            divulged_only: node == 1 || node == 3,
        })
        .collect();
        let page = LineagePage {
            delivery: ExercisePage {
                batch: Batch {
                    context: plan.context().clone(),
                    position: "page".into(),
                    events: vec![],
                },
                exercises,
            },
            creations,
        };
        (plan, page)
    }
    #[test]
    fn direct_parent_proofs_require_complete_subtrees_and_keep_all_visibility_inputs() {
        let (plan, mut page) = fixture();
        page.creations[1].witnesses = ["B".into()].into();
        let edges = prove(&plan, &page).unwrap();
        assert_eq!(
            edges
                .iter()
                .map(|e| (e.parent_node, e.child_node))
                .collect::<Vec<_>>(),
            vec![(0, 1), (2, 3), (2, 5), (0, 6)]
        );
        assert!(
            edges[3]
                .visibility_clauses
                .contains(&BTreeSet::from(["B".into()]))
        );
        page.creations.remove(1);
        assert!(prove(&plan, &page).unwrap().is_empty()); // Never upgrade the nearest visible exercise.
    }
    #[test]
    fn unknown_bounds_do_not_become_leaf_nodes_and_contradictory_identity_is_rejected() {
        let (plan, page) = fixture();
        let mut unknown = page.clone();
        unknown.delivery.exercises[1].last_descendant_node_id = -1;
        assert!(prove(&plan, &unknown).unwrap().is_empty());
        let mut crossing = page.clone();
        crossing.delivery.exercises[1].last_descendant_node_id = 7;
        assert_eq!(prove(&plan, &crossing), Err(EvidenceError));
        let mut mixed = page.clone();
        mixed.creations[0].transaction_id = "other".into();
        assert_eq!(prove(&plan, &mixed), Err(EvidenceError));
        let mut duplicate = page.clone();
        duplicate.creations[0].node_id = 2;
        assert_eq!(prove(&plan, &duplicate), Err(EvidenceError));
        let mut repeated = page.clone();
        repeated.creations[1].contract = repeated.creations[0].contract.clone();
        assert_eq!(prove(&plan, &repeated), Err(EvidenceError));
        let mut self_edge = page.clone();
        self_edge.creations[0].contract = self_edge.delivery.exercises[0].contract.clone();
        assert_eq!(prove(&plan, &self_edge), Err(EvidenceError));
        let mut huge = page;
        huge.delivery.exercises[0].last_descendant_node_id = i32::MAX;
        assert!(
            prove(&plan, &huge)
                .unwrap()
                .iter()
                .all(|e| e.parent_node != 0)
        );
    }
}
