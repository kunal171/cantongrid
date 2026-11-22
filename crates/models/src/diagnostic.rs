//! Sanitized evaluation summaries contain identities and finite codes, never payloads or field paths.
use crate::{Model, ModelError};
use serde::Serialize;

/// A local evaluator finding, not proof that an indexed projection or ledger transaction succeeded.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDiagnostic {
    pub model: String,
    pub revision: String,
    pub fingerprint: String,
    pub status: &'static str,
    pub error_code: Option<&'static str>,
}

impl Model {
    /// Reuse the exact bounded evaluator and discard all values before constructing public evidence.
    pub fn diagnose(&self, payload: &str) -> Result<ModelDiagnostic, ModelError> {
        let fingerprint = self.fingerprint()?;
        let (status, error_code) = match self.evaluate(payload) {
            Ok(Some(_)) => ("matched", None),
            Ok(None) => ("filtered", None),
            Err(error) => (
                "invalid",
                Some(match error {
                    ModelError::Document => "document",
                    ModelError::Definition => "definition",
                    ModelError::Type => "type",
                    ModelError::Path => "path",
                    ModelError::Missing => "missing",
                    ModelError::Overflow => "overflow",
                    ModelError::Limit => "limit",
                    ModelError::Query => "query",
                }),
            ),
        };
        Ok(ModelDiagnostic {
            model: self.name.clone(),
            revision: self.revision.to_string(),
            fingerprint,
            status,
            error_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_never_echo_secret_inputs_values_or_parser_details() {
        let model = crate::Catalog::parse(include_str!("../assets/models/visibility.json"))
            .unwrap()
            .models()[0]
            .clone();
        for payload in [
            "secret-sentinel-invalid-json",
            "{\"label\":\"secret-sentinel\"}",
            "{\"amount\":1.2}",
        ] {
            let finding = model.diagnose(payload).unwrap();
            assert_eq!(finding.status, "invalid");
            let encoded = serde_json::to_string(&finding).unwrap();
            assert!(!encoded.contains("secret-sentinel"));
            assert!(!encoded.contains("line "));
            assert!(!encoded.contains("amount"));
        }
        let mut payload: serde_json::Value =
            serde_json::from_str(include_str!("../assets/models/visibility-payload.json")).unwrap();
        assert_eq!(
            model.diagnose(&payload.to_string()).unwrap().status,
            "matched"
        );
        payload["state"] = "Other".into();
        assert_eq!(
            model.diagnose(&payload.to_string()).unwrap().status,
            "filtered"
        );
    }
}
