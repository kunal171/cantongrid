//! Versioned persistence envelopes. Decoding never authenticates a source or caller.
use crate::{
    config::{Plan, SourceContext, TemplateId},
    lifecycle::{Action, Batch, Event},
    value::Value,
    visibility::PartyId,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

/// Wire-only identities are validated when normalized; no trusted grant type is deserialized.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Context {
    source: String,
    network: String,
    generation: u64,
}

/// One ordered source delivery, without falsely claiming that a single batch is complete history.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    context: Context,
    position: String,
    events: Vec<WireEvent>,
}

/// An untrusted event carries exact payload values and the original source witness evidence.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireEvent {
    id: String,
    contract: String,
    template: TemplateId,
    witnesses: Vec<String>,
    observed_at_micros: i64,
    ledger_at_micros: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<crate::lifecycle::Origin>,
    action: WireAction,
}

/// Explicit tags prevent accidentally interpreting an unsupported event as a create or archive.
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireAction {
    Create { payload: Value },
    Archive,
    ArchiveByPriorContract,
}

impl Batch {
    /// Encode original evidence only. Visibility policies are recomputed during validated replay.
    pub fn encode(&self) -> Result<String, EnvelopeError> {
        let (source, network, generation) = self.context.identity();
        let events = self
            .events
            .iter()
            .map(|event| WireEvent {
                id: event.id.clone(),
                contract: event.contract.clone(),
                template: event.template.clone(),
                witnesses: event
                    .witnesses
                    .iter()
                    .map(|p| p.as_str().to_owned())
                    .collect(),
                observed_at_micros: event.observed_at_micros,
                ledger_at_micros: event.ledger_at_micros,
                origin: event.origin.clone(),
                action: match &event.action {
                    Action::Create(payload) => WireAction::Create {
                        payload: payload.clone(),
                    },
                    Action::Archive => WireAction::Archive,
                    Action::ArchiveByPriorContract => WireAction::ArchiveByPriorContract,
                },
            })
            .collect();
        serde_json::to_string(&Envelope {
            version: 1,
            context: Context {
                source: source.into(),
                network: network.into(),
                generation,
            },
            position: self.position.clone(),
            events,
        })
        .map_err(|_| EnvelopeError)
    }

    /// Decode under the expected plan. The lifecycle engine must validate transitions before use.
    pub fn decode(text: &str, plan: &Plan) -> Result<Self, EnvelopeError> {
        let wire: Envelope = crate::document::parse(text).map_err(|_| EnvelopeError)?;
        if wire.version != 1 {
            return Err(EnvelopeError);
        }
        let context = SourceContext::new(
            wire.context.source,
            wire.context.network,
            wire.context.generation,
        )
        .map_err(|_| EnvelopeError)?;
        if context != *plan.context() {
            return Err(EnvelopeError);
        }
        let events = wire
            .events
            .into_iter()
            .map(|event| {
                let count = event.witnesses.len();
                let witnesses: BTreeSet<_> = event
                    .witnesses
                    .into_iter()
                    .map(PartyId::new)
                    .collect::<Result<_, _>>()
                    .map_err(|_| EnvelopeError)?;
                if witnesses.len() != count {
                    return Err(EnvelopeError);
                }
                Ok(Event {
                    id: event.id,
                    contract: event.contract,
                    template: event.template,
                    witnesses,
                    observed_at_micros: event.observed_at_micros,
                    ledger_at_micros: event.ledger_at_micros,
                    origin: event.origin.clone(),
                    action: match event.action {
                        WireAction::Create { payload } => Action::Create(payload),
                        WireAction::Archive => Action::Archive,
                        WireAction::ArchiveByPriorContract => Action::ArchiveByPriorContract,
                    },
                })
            })
            .collect::<Result<_, EnvelopeError>>()?;
        Ok(Batch {
            context,
            position: wire.position,
            events,
        })
    }
}

/// Redacted failure: persisted payloads and identifiers never appear in diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeError;
impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or incompatible persisted batch envelope")
    }
}
impl std::error::Error for EnvelopeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    #[test]
    fn envelopes_round_trip_and_reject_conflicting_context_or_duplicate_keys() {
        let plan = Plan::parse(include_str!(
            "../../../examples/fixtures/visibility/cantongrid.json"
        ))
        .unwrap();
        let fixture = Fixture::parse(
            include_str!("../../../examples/fixtures/visibility/events.json"),
            &plan,
        )
        .unwrap();
        for batch in fixture.batches() {
            let encoded = batch.encode().unwrap();
            assert_eq!(&Batch::decode(&encoded, &plan).unwrap(), batch);
            let bad = encoded.replacen("\"version\":1", "\"version\":1,\"version\":2", 1);
            assert!(Batch::decode(&bad, &plan).is_err());
            let mut bad: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            bad["context"]["generation"] = 2.into();
            assert!(Batch::decode(&bad.to_string(), &plan).is_err());
        }
    }
    #[test]
    fn relocation_and_selection_order_do_not_change_the_persisted_definition() {
        let text = include_str!("../../../examples/fixtures/visibility/cantongrid.json");
        let original = Plan::parse(text).unwrap();
        let mut raw: serde_json::Value = serde_json::from_str(text).unwrap();
        raw["source"]["fixture"] = "different/location.json".into();
        raw["source"]["parties"] = serde_json::json!(["B", "A"]);
        assert_eq!(
            original.definition(),
            Plan::parse(&raw.to_string()).unwrap().definition()
        );
        raw["source"]["parties"] = serde_json::json!(["A"]);
        assert_ne!(
            original.definition(),
            Plan::parse(&raw.to_string()).unwrap().definition()
        );
    }
}
