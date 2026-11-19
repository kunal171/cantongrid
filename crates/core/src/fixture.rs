//! Credential-free adapter for explicitly synthetic complete-history fixtures.
use crate::{
    authorization::{MAX_SOURCE_LEASE, SourceLease},
    config::{Plan, SourceContext, TemplateId},
    lifecycle::{Action, Batch, Engine, Event},
    value::Value,
    visibility::{CallerGrants, PartyId},
};
use serde::Deserialize;
use std::{collections::BTreeSet, fmt, time::Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format_version: u32,
    model_version: u32,
    payload_schema_version: u32,
    coverage: Coverage,
    source: Source,
    batches: Vec<RawBatch>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Coverage {
    CompleteFromOrigin,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    network: String,
    generation: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBatch {
    position: String,
    events: Vec<RawEvent>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    id: String,
    contract: String,
    template: TemplateId,
    witnesses: Vec<String>,
    observed_at_micros: i64,
    ledger_at_micros: Option<i64>,
    action: RawAction,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawAction {
    Create { payload: Value },
    Archive,
}

pub struct Fixture {
    batches: Vec<Batch>,
}
impl Fixture {
    pub fn parse(text: &str, plan: &Plan) -> Result<Self, FixtureError> {
        let document: Document =
            crate::document::parse(text).map_err(|_| FixtureError::Document)?;
        if document.format_version != 1
            || document.model_version != 1
            || document.payload_schema_version != 1
        {
            return Err(FixtureError::Version);
        }
        let Coverage::CompleteFromOrigin = document.coverage;
        let source = document.source;
        let context = SourceContext::new(source.id, source.network, source.generation)
            .map_err(|_| FixtureError::Context)?;
        if context != *plan.context() {
            return Err(FixtureError::Context);
        }
        let mut batches = vec![];
        for raw in document.batches {
            let mut events = vec![];
            for raw in raw.events {
                let count = raw.witnesses.len();
                let witnesses: BTreeSet<_> = raw
                    .witnesses
                    .into_iter()
                    .map(PartyId::new)
                    .collect::<Result<_, _>>()
                    .map_err(|_| FixtureError::Document)?;
                if witnesses.len() != count {
                    return Err(FixtureError::Document);
                }
                events.push(Event {
                    id: raw.id,
                    contract: raw.contract,
                    template: raw.template,
                    witnesses,
                    observed_at_micros: raw.observed_at_micros,
                    ledger_at_micros: raw.ledger_at_micros,
                    action: match raw.action {
                        RawAction::Create { payload } => Action::Create(payload),
                        RawAction::Archive => Action::Archive,
                    },
                });
            }
            batches.push(Batch {
                context: context.clone(),
                position: raw.position,
                events,
            });
        }
        Ok(Self { batches })
    }

    /// Ordered source batches, shared by the in-memory runner and durable fixture ingestion.
    pub fn batches(&self) -> &[Batch] {
        &self.batches
    }

    pub fn replay_into(&self, engine: &mut Engine) -> Result<usize, FixtureError> {
        let mut applied = 0;
        for batch in &self.batches {
            applied += usize::from(engine.apply(batch.clone()).map_err(FixtureError::Ingest)?);
        }
        Ok(applied)
    }

    /// Simulated fixture grants only. This is not a production authentication boundary.
    pub fn run(&self, plan: &Plan, parties: Vec<String>) -> Result<Summary, FixtureError> {
        let caller_parties: BTreeSet<_> = parties
            .into_iter()
            .map(PartyId::new)
            .collect::<Result<_, _>>()
            .map_err(|_| FixtureError::Document)?;
        if !caller_parties.is_subset(&plan.parties) {
            return Err(FixtureError::Caller);
        }
        let mut engine = Engine::new(plan.clone());
        let applied_batches = self.replay_into(&mut engine)?;
        let replayed = self.replay_into(&mut engine)?;
        if replayed != 0 {
            return Err(FixtureError::Replay);
        }
        let now = Instant::now();
        let lease = SourceLease::from_verified(
            plan.context.binding(),
            plan.parties.iter().cloned(),
            now,
            MAX_SOURCE_LEASE,
            now + MAX_SOURCE_LEASE,
        )
        .map_err(|_| FixtureError::Caller)?;
        let caller = CallerGrants::from_verified(plan.context.binding(), caller_parties);
        Ok(Summary {
            applied_batches,
            replay_applied_batches: replayed,
            visible_active: engine.active(&caller, &lease, now).len(),
            visible_history: engine.history(&caller, &lease, now).len(),
            has_checkpoint: engine.checkpoint().is_some(),
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct Summary {
    pub applied_batches: usize,
    pub replay_applied_batches: usize,
    pub visible_active: usize,
    pub visible_history: usize,
    pub has_checkpoint: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureError {
    Document,
    Version,
    Context,
    Ingest(crate::lifecycle::IngestError),
    Caller,
    Replay,
}
impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ingest(error) => error.fmt(f),
            _ => f.write_str(match self {
                Self::Document => "invalid fixture document; check required fields, typed values and duplicate keys",
                Self::Version => "unsupported fixture format/model/payload schema version",
                Self::Context => "fixture source/network/generation must match the project",
                Self::Caller => "fixture caller parties must be within the declared source scope",
                Self::Replay => "fixture replay unexpectedly changed state",
                Self::Ingest(_) => unreachable!(),
            }),
        }
    }
}
impl std::error::Error for FixtureError {}

#[cfg(test)]
mod tests {
    use super::*;
    const PROJECT: &str = include_str!("../assets/fixtures/visibility/cantongrid.json");
    const EVENTS: &str = include_str!("../assets/fixtures/visibility/events.json");
    #[test]
    fn examples_share_one_runner_and_preserve_party_isolation() {
        let plan = Plan::parse(PROJECT).unwrap();
        let fixture = Fixture::parse(EVENTS, &plan).unwrap();
        for (parties, active, history) in [
            (vec!["A"], 2, 4),
            (vec!["B"], 2, 2),
            (vec!["A", "B"], 3, 5),
            (vec![], 0, 0),
        ] {
            let result = fixture
                .run(&plan, parties.into_iter().map(String::from).collect())
                .unwrap();
            assert_eq!(result.visible_active, active);
            assert_eq!(result.visible_history, history);
            assert_eq!(result.replay_applied_batches, 0);
            assert!(result.has_checkpoint);
        }
        let plan =
            Plan::parse(include_str!("../assets/fixtures/outcomia/cantongrid.json")).unwrap();
        let fixture = Fixture::parse(
            include_str!("../assets/fixtures/outcomia/events.json"),
            &plan,
        )
        .unwrap();
        let result = fixture.run(&plan, vec!["A".into()]).unwrap();
        assert_eq!(result.visible_active, 1);
        assert_eq!(result.visible_history, 3);
    }
    #[test]
    fn mismatched_context_versions_coverage_and_unknown_events_fail() {
        let plan = Plan::parse(PROJECT).unwrap();
        for (field, value) in [
            ("format_version", serde_json::json!(2)),
            ("coverage", serde_json::json!("pruned")),
        ] {
            let mut input: serde_json::Value = serde_json::from_str(EVENTS).unwrap();
            input[field] = value;
            assert!(Fixture::parse(&input.to_string(), &plan).is_err());
        }
        let mut input: serde_json::Value = serde_json::from_str(EVENTS).unwrap();
        input["source"]["generation"] = 2.into();
        assert!(matches!(
            Fixture::parse(&input.to_string(), &plan),
            Err(FixtureError::Context)
        ));
        input = serde_json::from_str(EVENTS).unwrap();
        input["batches"][0]["events"][0]["action"]["kind"] = "exercise".into();
        assert!(Fixture::parse(&input.to_string(), &plan).is_err());
        assert!(
            Fixture::parse(EVENTS, &plan)
                .unwrap()
                .run(&plan, vec!["C".into()])
                .is_err()
        );
    }
    #[test]
    fn parse_rejects_duplicate_keys_without_secret_diagnostics() {
        assert!(
            Plan::parse(&PROJECT.replacen("\"version\": 1", "\"version\": 1, \"version\": 2", 1))
                .is_err()
        );
        let plan = Plan::parse(PROJECT).unwrap();
        let duplicated = EVENTS.replacen(
            "\"coverage\":",
            "\"coverage\": \"PRIVATE\", \"coverage\":",
            1,
        );
        let error = Fixture::parse(&duplicated, &plan).err().unwrap();
        assert!(!error.to_string().contains("PRIVATE"));
    }
}
