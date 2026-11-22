//! Reproducible operator-supplied samples exercise the existing projection evaluator, not LF code.
use crate::{SyncError as Error, SyncPlan};
use cantongrid_models::ModelDiagnostic;
use serde::{Deserialize, Serialize};

/// Bounded samples have no arbitrary labels to accidentally echo a private input identifier.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Samples {
    format_version: u32,
    samples: Vec<Sample>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    model: String,
    before: serde_json::Value,
    after: serde_json::Value,
}
/// Values are compared in memory and discarded. Missing candidate evaluations mean blocked plans.
#[derive(Serialize)]
pub struct EvaluationReport {
    pub format_version: u32,
    pub compatible: bool,
    pub samples: Vec<Evaluation>,
}
/// One-based input ordinal identifies a sample without retaining operator-supplied labels.
#[derive(Serialize)]
pub struct Evaluation {
    pub ordinal: usize,
    pub before: ModelDiagnostic,
    pub after: Option<ModelDiagnostic>,
    pub projected_values_equal: Option<bool>,
}
impl SyncPlan {
    /// Validate the complete sample document before evaluation. Identical payloads are not assumed:
    /// e.g. a new optional field can be absent in `before` and explicitly null in `after`.
    pub fn evaluate(&self, fixtures_json: &str) -> Result<EvaluationReport, Error> {
        if fixtures_json.len() > 65536 {
            return Err(Error::Limit);
        }
        let fixtures: Samples =
            cantongrid_core::document::parse(fixtures_json).map_err(|_| Error::Definition)?;
        if fixtures.format_version != 1
            || fixtures.samples.is_empty()
            || fixtures.samples.len() > 32
        {
            return Err(Error::Limit);
        }
        for sample in &fixtures.samples {
            if !sample.before.is_object()
                || !sample.after.is_object()
                || !self.previous.iter().any(|m| m.name == sample.model)
            {
                return Err(Error::Definition);
            }
        }
        let mut samples = Vec::new();
        for (ordinal, sample) in fixtures.samples.iter().enumerate() {
            let model = self
                .previous
                .iter()
                .find(|m| m.name == sample.model)
                .ok_or(Error::Definition)?;
            let before_payload = sample.before.to_string();
            let after_payload = sample.after.to_string();
            let before = model
                .diagnose(&before_payload)
                .map_err(|_| Error::Definition)?;
            let candidate = self
                .catalog
                .as_ref()
                .and_then(|c| c.models().iter().find(|m| m.name == sample.model));
            let after = candidate
                .map(|m| m.diagnose(&after_payload))
                .transpose()
                .map_err(|_| Error::Definition)?;
            let projected_values_equal = candidate.and_then(|m| {
                match (model.evaluate(&before_payload), m.evaluate(&after_payload)) {
                    (Ok(a), Ok(b)) => Some(a == b),
                    _ => None,
                }
            });
            samples.push(Evaluation {
                ordinal: ordinal + 1,
                before,
                after,
                projected_values_equal,
            });
        }
        Ok(EvaluationReport {
            format_version: 1,
            compatible: self.report.compatible,
            samples,
        })
    }
}
