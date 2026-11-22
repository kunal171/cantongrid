//! Version-one lifecycle transport. Raw contract JSON stays explicit; financial values stay strings.
use serde::{Deserialize, Serialize};

/// Canonical lifecycle kinds only. Exercises and model projection changes are separate event domains.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryKind {
    Create,
    Archive,
}
/// Public evidence vocabulary identifies a policy without exposing its witnesses or dependency sets.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryEvidence {
    SourceWitnessPolicy,
    PriorCreatePolicy,
}
/// A fixed public record shape strips raw envelopes, source PKs, arguments and controller metadata.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DeliveryEvent {
    pub event_id: String,
    pub contract_id: String,
    pub kind: DeliveryKind,
    pub template: cantongrid_core::config::TemplateId,
    pub payload: Option<cantongrid_core::value::Value>,
    pub observed_at_micros: Option<String>,
    pub ledger_at_micros: Option<String>,
    pub source_offset: Option<String>,
    pub snapshot_observation: Option<bool>,
    pub visibility_evidence: DeliveryEvidence,
}
