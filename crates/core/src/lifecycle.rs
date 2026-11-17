//! Source-neutral, in-memory lifecycle reference engine. No durable checkpoint claim.
use crate::{
    authorization::SourceLease,
    config::{Plan, SourceContext, TemplateId, identifier},
    value::Value,
    visibility::{CallerGrants, PartyId, VisibilityPolicy},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    time::Instant,
};

#[derive(Clone, PartialEq, Eq)]
pub enum Action {
    Create(Value),
    Archive,
    /// Source archive has no witnesses; access depends on the previously observed creation.
    ArchiveByPriorContract,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub contract: String,
    pub template: TemplateId,
    pub witnesses: BTreeSet<PartyId>,
    pub observed_at_micros: i64,
    pub ledger_at_micros: Option<i64>,
    pub action: Action,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Batch {
    pub context: SourceContext,
    /// Opaque source position. Only fixture delivery order establishes ordering here.
    pub position: String,
    pub events: Vec<Event>,
}

impl fmt::Debug for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Batch([redacted])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Contract {
    pub id: String,
    pub template: TemplateId,
    pub payload: Value,
    pub created_by: String,
    pub position: String,
    policy: VisibilityPolicy,
    active: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Observation {
    pub event: Event,
    pub position: String,
    policy: VisibilityPolicy,
}

#[derive(Clone)]
pub struct Engine {
    plan: Plan,
    contracts: BTreeMap<String, Contract>,
    history: Vec<Observation>,
    batches: BTreeMap<String, Batch>,
    event_positions: BTreeMap<String, (String, Event)>,
    checkpoint: Option<String>,
}

impl Engine {
    pub fn new(plan: Plan) -> Self {
        Self {
            plan,
            contracts: BTreeMap::new(),
            history: vec![],
            batches: BTreeMap::new(),
            event_positions: BTreeMap::new(),
            checkpoint: None,
        }
    }

    /// Validate and stage a whole source batch before publishing any state/progress.
    /// A future database adapter must implement this transaction boundary durably.
    pub fn apply(&mut self, batch: Batch) -> Result<bool, IngestError> {
        if batch.context != self.plan.context {
            return Err(IngestError::Context);
        }
        if !identifier(&batch.position) {
            return Err(IngestError::InvalidEvent);
        }
        if let Some(previous) = self.batches.get(&batch.position) {
            return if *previous == batch {
                Ok(false)
            } else {
                Err(IngestError::Conflict)
            };
        }
        let mut staged = self.clone();
        for event in &batch.events {
            staged.apply_event(event, &batch.position)?;
        }
        staged.checkpoint = Some(batch.position.clone());
        staged.batches.insert(batch.position.clone(), batch);
        *self = staged;
        Ok(true)
    }

    fn apply_event(&mut self, event: &Event, position: &str) -> Result<(), IngestError> {
        if !identifier(&event.id)
            || !identifier(&event.contract)
            || event.template.validate().is_err()
            || event.observed_at_micros < 0
            || event.ledger_at_micros.is_some_and(|time| time < 0)
        {
            return Err(IngestError::InvalidEvent);
        }
        let inherited = matches!(event.action, Action::ArchiveByPriorContract);
        if (inherited && (!self.plan.is_live() || !event.witnesses.is_empty()))
            || (!inherited
                && (event.witnesses.is_empty() || event.witnesses.is_disjoint(&self.plan.parties)))
        {
            return Err(IngestError::Visibility);
        }
        if let Action::Create(value) = &event.action {
            value.validate().map_err(|_| IngestError::Value)?;
        }
        if let Some((previous_position, previous_event)) = self.event_positions.get(&event.id) {
            return if previous_position == position && previous_event == event {
                Ok(())
            } else {
                Err(IngestError::Conflict)
            };
        }
        self.event_positions
            .insert(event.id.clone(), (position.into(), event.clone()));
        if !self.plan.templates.contains(&event.template) {
            return Ok(());
        }
        let policy = if inherited {
            self.contracts
                .get(&event.contract)
                .ok_or(IngestError::Lifecycle)?
                .policy
                .clone()
        } else {
            VisibilityPolicy::from_witnesses(
                self.plan.context.binding(),
                event.witnesses.iter().cloned(),
            )
            .map_err(|_| IngestError::Visibility)?
        };
        let policy = match &event.action {
            Action::Create(payload) => {
                if self.contracts.contains_key(&event.contract) {
                    return Err(IngestError::Lifecycle);
                }
                self.contracts.insert(
                    event.contract.clone(),
                    Contract {
                        id: event.contract.clone(),
                        template: event.template.clone(),
                        payload: payload.clone(),
                        created_by: event.id.clone(),
                        position: position.into(),
                        policy: policy.clone(),
                        active: true,
                    },
                );
                policy
            }
            Action::Archive | Action::ArchiveByPriorContract => {
                let contract = self
                    .contracts
                    .get_mut(&event.contract)
                    .ok_or(IngestError::Lifecycle)?;
                if !contract.active || contract.template != event.template {
                    return Err(IngestError::Lifecycle);
                }
                // Archival evidence cannot widen access beyond the original contract evidence.
                let combined = VisibilityPolicy::join(&[contract.policy.clone(), policy])
                    .map_err(|_| IngestError::Visibility)?;
                contract.active = false;
                combined
            }
        };
        self.history.push(Observation {
            event: event.clone(),
            position: position.into(),
            policy,
        });
        Ok(())
    }

    pub fn active<'a>(
        &'a self,
        caller: &CallerGrants,
        lease: &SourceLease,
        now: Instant,
    ) -> Vec<&'a Contract> {
        self.contracts
            .values()
            .filter(|c| c.active && lease.authorize(&c.policy, caller, now))
            .collect()
    }

    pub fn history<'a>(
        &'a self,
        caller: &CallerGrants,
        lease: &SourceLease,
        now: Instant,
    ) -> Vec<&'a Observation> {
        self.history
            .iter()
            .filter(|o| lease.authorize(&o.policy, caller, now))
            .collect()
    }

    /// Operator progress, never an API cursor or a universal ledger height.
    pub fn checkpoint(&self) -> Option<&str> {
        self.checkpoint.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IngestError {
    Context,
    InvalidEvent,
    Visibility,
    Value,
    Conflict,
    Lifecycle,
}
impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Context => "batch source/network/generation differs from the validated plan",
            Self::InvalidEvent => "invalid event identity, position, template or timestamp",
            Self::Visibility => "missing witnesses or event outside the declared source parties",
            Self::Value => "invalid or unsupported typed payload",
            Self::Conflict => "conflicting delivery at an existing event identity or position",
            Self::Lifecycle => "invalid lifecycle transition: expected a unique create followed by at most one matching archive",
        })
    }
}
impl std::error::Error for IngestError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authorization::MAX_SOURCE_LEASE;
    fn plan() -> Plan {
        Plan::parse(&serde_json::json!({"version":1,"model_version":1,"payload_schema_version":1,
            "source":{"adapter":"fixture","id":"test","network":"synthetic","generation":1,
                "parties":["A","B"],"fixture":"events.json"},
            "selection":{"templates":[{"package_id":"0".repeat(64),"module":"Demo","entity":"Record"}],
                "events":["create","archive"],"require_complete_history":true}}).to_string()).unwrap()
    }
    fn event(id: &str, contract: &str, parties: &[&str], action: Action) -> Event {
        Event {
            id: id.into(),
            contract: contract.into(),
            template: plan().templates.first().unwrap().clone(),
            witnesses: parties.iter().map(|p| PartyId::new(*p).unwrap()).collect(),
            observed_at_micros: 1,
            ledger_at_micros: Some(1),
            action,
        }
    }
    fn create(id: &str, parties: &[&str]) -> Event {
        event(
            id,
            id,
            parties,
            Action::Create(Value::Text("synthetic".into())),
        )
    }
    fn batch(position: &str, events: Vec<Event>) -> Batch {
        Batch {
            context: plan().context,
            position: position.into(),
            events,
        }
    }
    fn caller(parties: &[&str]) -> CallerGrants {
        CallerGrants::from_verified(
            plan().context.binding(),
            parties.iter().map(|p| PartyId::new(*p).unwrap()),
        )
    }
    fn lease(now: Instant) -> SourceLease {
        SourceLease::from_verified(
            plan().context.binding(),
            plan().parties,
            now,
            MAX_SOURCE_LEASE,
            now + MAX_SOURCE_LEASE,
        )
        .unwrap()
    }
    #[test]
    fn replay_archive_and_scoped_reads_are_deterministic() {
        let first = batch(
            "opaque-z",
            vec![
                create("a", &["A"]),
                create("b", &["B"]),
                create("joint", &["A", "B"]),
            ],
        );
        let second = batch(
            "opaque-a",
            vec![event("archive-a", "a", &["A"], Action::Archive)],
        );
        let mut engine = Engine::new(plan());
        engine.apply(first.clone()).unwrap();
        engine.apply(second.clone()).unwrap();
        let now = Instant::now();
        let mut lease = lease(now);
        for (parties, active, history) in [
            (&["A"][..], 1, 3),
            (&["B"][..], 2, 2),
            (&["A", "B"][..], 2, 4),
            (&[][..], 0, 0),
        ] {
            assert_eq!(engine.active(&caller(parties), &lease, now).len(), active);
            assert_eq!(engine.history(&caller(parties), &lease, now).len(), history);
        }
        assert!(!engine.apply(first.clone()).unwrap());
        assert!(!engine.apply(second.clone()).unwrap());
        assert_eq!(engine.checkpoint(), Some("opaque-a")); // no lexical position ordering
        let mut replay = Engine::new(plan());
        replay.apply(first).unwrap();
        replay.apply(second).unwrap();
        assert!(replay.contracts == engine.contracts && replay.history == engine.history);
        assert!(
            engine
                .active(&caller(&["A", "B"]), &lease, now + MAX_SOURCE_LEASE)
                .is_empty()
        );
        lease.invalidate();
        assert!(engine.history(&caller(&["A", "B"]), &lease, now).is_empty());
    }
    #[test]
    fn rejected_batches_leave_no_partial_state_or_progress() {
        let mut engine = Engine::new(plan());
        engine
            .apply(batch("one", vec![create("existing", &["A"])]))
            .unwrap();
        for bad in [
            event("unknown-archive", "unknown", &["A"], Action::Archive),
            create("existing", &["A"]),
            create("empty-witness", &[]),
            create("outside", &["C"]),
            event(
                "bad-value",
                "v",
                &["A"],
                Action::Create(Value::Int64("9223372036854775808".into())),
            ),
        ] {
            assert!(
                engine
                    .apply(batch("two", vec![create("staged", &["A"]), bad]))
                    .is_err()
            );
            assert_eq!(engine.checkpoint(), Some("one"));
            assert_eq!(engine.contracts.len(), 1);
            assert_eq!(engine.history.len(), 1);
            assert!(!engine.event_positions.contains_key("staged"));
        }
    }
    #[test]
    fn identities_and_context_cannot_be_reused_for_conflicting_content() {
        let mut engine = Engine::new(plan());
        let first = batch("one", vec![create("a", &["A"])]);
        engine.apply(first.clone()).unwrap();
        assert_eq!(
            engine.apply(batch("one", vec![create("b", &["B"])])),
            Err(IngestError::Conflict)
        );
        assert_eq!(
            engine.apply(batch("two", first.events.clone())),
            Err(IngestError::Conflict)
        );
        let mut different = first.clone();
        different.context = SourceContext::new("test".into(), "other-network".into(), 1).unwrap();
        assert_eq!(engine.apply(different), Err(IngestError::Context));
        let mut different = first;
        different.context = SourceContext::new("test".into(), "synthetic".into(), 2).unwrap();
        assert_eq!(engine.apply(different), Err(IngestError::Context));
        assert_eq!(engine.checkpoint(), Some("one"));
    }
    #[test]
    fn archives_do_not_widen_original_visibility_or_invent_unknown_creates() {
        let mut engine = Engine::new(plan());
        engine
            .apply(batch("one", vec![create("a", &["A"])]))
            .unwrap();
        engine
            .apply(batch(
                "two",
                vec![event("archive", "a", &["B"], Action::Archive)],
            ))
            .unwrap();
        let now = Instant::now();
        let lease = lease(now);
        assert_eq!(engine.history(&caller(&["B"]), &lease, now).len(), 0);
        assert_eq!(engine.history(&caller(&["A"]), &lease, now).len(), 1);
        assert_eq!(engine.history(&caller(&["A", "B"]), &lease, now).len(), 2);
        assert!(
            engine
                .apply(batch(
                    "three",
                    vec![event("again", "a", &["A"], Action::Archive)]
                ))
                .is_err()
        );
        let mut recreate = create("new-id", &["A"]);
        recreate.contract = "a".into();
        assert!(engine.apply(batch("three", vec![recreate])).is_err());
    }
    #[test]
    fn exact_template_selection_does_not_infer_archival_from_absence() {
        let mut engine = Engine::new(plan());
        let mut other = create("ignored", &["A"]);
        other.template.entity = "Unselected".into();
        engine
            .apply(batch("one", vec![create("a", &["A"]), other]))
            .unwrap();
        engine.apply(batch("empty", vec![])).unwrap();
        assert_eq!(engine.contracts.len(), 1);
        assert!(engine.contracts["a"].active);
        assert_eq!(engine.history.len(), 1);
    }
}

#[cfg(test)]
mod live_policy_tests {
    use super::*;
    #[test]
    fn empty_archive_evidence_requires_prior_create_and_live_profile() {
        let context =
            SourceContext::new("scope-policy-test".into(), "synthetic".into(), 1).unwrap();
        let party = PartyId::new("A").unwrap();
        let template = TemplateId {
            package_id: "0".repeat(64),
            module: "Demo".into(),
            entity: "Record".into(),
        };
        let plan = Plan::pqs(
            context.clone(),
            BTreeSet::from([party.clone()]),
            BTreeSet::from([template.clone()]),
        )
        .unwrap();
        let create = Event {
            id: "create".into(),
            contract: "contract".into(),
            template,
            witnesses: BTreeSet::from([party.clone()]),
            observed_at_micros: 1,
            ledger_at_micros: None,
            action: Action::Create(Value::RawJson(
                r#"{"amount":"12345678901234567890.1234567891","sequence":"9223372036854775807"}"#
                    .into(),
            )),
        };
        let archive = Event {
            id: "archive".into(),
            witnesses: BTreeSet::new(),
            action: Action::ArchiveByPriorContract,
            ..create.clone()
        };
        let mut engine = Engine::new(plan.clone());
        assert!(
            engine
                .apply(Batch {
                    context: context.clone(),
                    position: "orphan".into(),
                    events: vec![archive.clone()]
                })
                .is_err()
        );
        engine
            .apply(Batch {
                context: context.clone(),
                position: "1".into(),
                events: vec![create],
            })
            .unwrap();
        let batch = Batch {
            context: context.clone(),
            position: "2".into(),
            events: vec![archive],
        };
        let decoded = Batch::decode(&batch.encode().unwrap(), &plan).unwrap();
        assert!(decoded.events[0].witnesses.is_empty());
        engine.apply(decoded).unwrap();
        let now = Instant::now();
        let lease = SourceLease::from_verified(
            context.binding(),
            [party.clone()],
            now,
            crate::authorization::MAX_SOURCE_LEASE,
            now + std::time::Duration::from_secs(60),
        )
        .unwrap();
        let a = CallerGrants::from_verified(context.binding(), [party]);
        let b = CallerGrants::from_verified(context.binding(), [PartyId::new("B").unwrap()]);
        assert!(engine.active(&a, &lease, now).is_empty());
        assert_eq!(engine.history(&a, &lease, now).len(), 2);
        assert!(engine.history(&b, &lease, now).is_empty());
    }
}
