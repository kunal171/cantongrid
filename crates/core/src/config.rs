//! Strict, versioned fixture configuration. Live credentials/adapters are not accepted yet.
use crate::visibility::{PartyId, ScopeBinding};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, path::PathBuf};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateId {
    pub package_id: String,
    pub module: String,
    pub entity: String,
}

impl TemplateId {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.package_id.len() != 64
            || !self
                .package_id
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || !name(&self.module, true)
            || !name(&self.entity, false)
        {
            return Err(ConfigError::InvalidTemplate);
        }
        Ok(())
    }
}

impl fmt::Debug for TemplateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TemplateId([redacted])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourceContext {
    source: String,
    network: String,
    generation: u64,
}

impl fmt::Debug for SourceContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceContext([redacted])")
    }
}

impl SourceContext {
    pub fn new(source: String, network: String, generation: u64) -> Result<Self, ConfigError> {
        if !identifier(&source) || !identifier(&network) || generation == 0 {
            return Err(ConfigError::InvalidSource);
        }
        Ok(Self {
            source,
            network,
            generation,
        })
    }

    /// Stable identity fields for persistence; these identify a source, not permission to read it.
    pub fn identity(&self) -> (&str, &str, u64) {
        (&self.source, &self.network, self.generation)
    }

    pub fn binding(&self) -> ScopeBinding {
        // Length-prefixing prevents separator collisions between network and source.
        ScopeBinding::new(
            format!("{}:{}{}", self.network.len(), self.network, self.source),
            self.generation,
        )
        .expect("validated source identity")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Create,
    Archive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub creates: bool,
    pub archives: bool,
    pub complete_history: bool,
}

impl Capabilities {
    pub const FIXTURE: Self = Self {
        creates: true,
        archives: true,
        complete_history: true,
    };

    pub fn require(&self, events: &[EventKind], complete_history: bool) -> Result<(), ConfigError> {
        if events.iter().any(|kind| match kind {
            EventKind::Create => !self.creates,
            EventKind::Archive => !self.archives,
        }) || (complete_history && !self.complete_history)
        {
            return Err(ConfigError::UnsupportedCapability);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    model_version: u32,
    payload_schema_version: u32,
    source: Source,
    selection: Selection,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    adapter: Adapter,
    id: String,
    network: String,
    generation: u64,
    parties: Vec<String>,
    fixture: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Adapter {
    Fixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    templates: Vec<TemplateId>,
    events: Vec<EventKind>,
    require_complete_history: bool,
}

/// Constructed only through validation, not directly deserializable as a trusted plan.
#[derive(Clone)]
pub struct Plan {
    pub(crate) context: SourceContext,
    pub(crate) parties: BTreeSet<PartyId>,
    pub(crate) templates: BTreeSet<TemplateId>,
    fixture: PathBuf,
    live: bool,
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Plan([redacted])")
    }
}

impl Plan {
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        // Never forward Serde's value-containing diagnostics into logs.
        let document: Document =
            crate::document::parse(input).map_err(|_| ConfigError::InvalidDocument)?;
        if document.version != FORMAT_VERSION
            || document.model_version != 1
            || document.payload_schema_version != 1
        {
            return Err(ConfigError::UnsupportedVersion);
        }
        Capabilities::FIXTURE.require(
            &document.selection.events,
            document.selection.require_complete_history,
        )?;
        // This slice maintains full lifecycle state; create-only selection would leave stale state.
        if document.selection.events.len() != 2
            || !document.selection.events.contains(&EventKind::Create)
            || !document.selection.events.contains(&EventKind::Archive)
            || !document.selection.require_complete_history
        {
            return Err(ConfigError::UnsupportedCapability);
        }
        let source = document.source;
        let Adapter::Fixture = source.adapter;
        let context = SourceContext::new(source.id, source.network, source.generation)?;
        if source.parties.is_empty() || source.fixture.as_os_str().is_empty() {
            return Err(ConfigError::InvalidSource);
        }
        let party_count = source.parties.len();
        let parties: BTreeSet<_> = source
            .parties
            .into_iter()
            .map(PartyId::new)
            .collect::<Result<_, _>>()
            .map_err(|_| ConfigError::InvalidSource)?;
        if parties.len() != party_count {
            return Err(ConfigError::InvalidSource);
        }
        let templates = document.selection.templates;
        if templates.is_empty() {
            return Err(ConfigError::InvalidTemplate);
        }
        for template in &templates {
            template.validate()?;
        }
        let count = templates.len();
        let templates: BTreeSet<_> = templates.into_iter().collect();
        if templates.len() != count {
            return Err(ConfigError::InvalidTemplate);
        }
        Ok(Self {
            context,
            parties,
            templates,
            fixture: source.fixture,
            live: false,
        })
    }

    /// Build the supported partial-history PQS profile from validated source configuration.
    /// It uses exact template projections and an explicit prior-create archive policy.
    pub fn pqs(
        context: SourceContext,
        parties: BTreeSet<PartyId>,
        templates: BTreeSet<TemplateId>,
    ) -> Result<Self, ConfigError> {
        if parties.is_empty() || parties.len() > 128 || templates.is_empty() || templates.len() > 64
        {
            return Err(ConfigError::InvalidSource);
        }
        for template in &templates {
            template.validate()?;
        }
        Ok(Self {
            context,
            parties,
            templates,
            fixture: PathBuf::new(),
            live: true,
        })
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    pub fn templates(&self) -> impl Iterator<Item = &TemplateId> {
        self.templates.iter()
    }

    pub fn selected_template_count(&self) -> usize {
        self.templates.len()
    }
    pub fn party_count(&self) -> usize {
        self.parties.len()
    }

    /// Canonical selection definition binds persisted state to exactly this plan.
    /// Fixture file location is excluded: relocating an identical input must not reset progress.
    pub fn definition(&self) -> serde_json::Value {
        let (source, network, generation) = self.context.identity();
        let mut definition = serde_json::json!({
            "format_version": 1, "model_version": 1, "payload_schema_version": 1,
            "source": source, "network": network, "generation": generation,
            "parties": self.parties.iter().map(PartyId::as_str).collect::<Vec<_>>(),
            "templates": self.templates,
        });
        if self.live {
            definition["profile"] = serde_json::json!("pqs041_partial_raw_prior_create_v1");
        }
        definition
    }

    /// Declared source parties are configuration, never proof of current source/caller permissions.
    pub fn source_parties(&self) -> impl Iterator<Item = &PartyId> {
        self.parties.iter()
    }

    /// Test exact template membership without exposing a mutable selection.
    pub fn selects(&self, template: &TemplateId) -> bool {
        self.templates.contains(template)
    }

    pub fn fixture_path(&self) -> &std::path::Path {
        &self.fixture
    }
    pub fn context(&self) -> &SourceContext {
        &self.context
    }
}

pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control) && value.trim() == value
}

fn name(value: &str, dotted: bool) -> bool {
    value.split('.').all(|part| {
        let mut chars = part.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }) && (dotted || !value.contains('.'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidDocument,
    UnsupportedVersion,
    InvalidSource,
    InvalidTemplate,
    UnsupportedCapability,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDocument => "invalid project document: check required fields, types and unknown fields",
            Self::UnsupportedVersion => "unsupported project, model or payload schema version (expected 1)",
            Self::InvalidSource => "source requires valid identity, network, positive generation, unique parties and fixture path",
            Self::InvalidTemplate => "selection requires unique exact package/module/entity templates",
            Self::UnsupportedCapability => "fixture state requires create and archive events with complete history",
        })
    }
}
impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> serde_json::Value {
        serde_json::json!({"version":1,"model_version":1,"payload_schema_version":1,
            "source":{"adapter":"fixture","id":"test","network":"synthetic","generation":1,
                "parties":["A","B"],"fixture":"events.json"},
            "selection":{"templates":[{"package_id":"0".repeat(64),"module":"Demo","entity":"Record"}],
                "events":["create","archive"],"require_complete_history":true}})
    }
    #[test]
    fn strict_config_and_redacted_errors() {
        assert!(Plan::parse(&valid().to_string()).is_ok());
        for (field, value) in [
            ("version", serde_json::json!(2)),
            ("secret-token", serde_json::json!("PRIVATE")),
        ] {
            let mut input = valid();
            input[field] = value;
            let error = Plan::parse(&input.to_string()).unwrap_err();
            assert!(!format!("{error:?}: {error}").contains("PRIVATE"));
        }
        let mut input = valid();
        input["source"]["adapter"] = "pqs".into();
        assert!(Plan::parse(&input.to_string()).is_err());
        input = valid();
        input["selection"]["events"] = serde_json::json!(["create"]);
        assert_eq!(
            Plan::parse(&input.to_string()).unwrap_err(),
            ConfigError::UnsupportedCapability
        );
    }
    #[test]
    fn invalid_scope_templates_and_unknown_processing_fail_closed() {
        for path in ["parties", "fixture", "generation"] {
            let mut input = valid();
            input["source"][path] = serde_json::Value::Null;
            assert!(Plan::parse(&input.to_string()).is_err());
        }
        let mut input = valid();
        input["source"]["parties"] = serde_json::json!(["A", "A"]);
        assert!(Plan::parse(&input.to_string()).is_err());
        input = valid();
        input["selection"]["templates"][0]["package_id"] = "*".into();
        assert!(Plan::parse(&input.to_string()).is_err());
        input = valid();
        input["models"] = serde_json::json!([{"operator":"sum"}]);
        assert!(Plan::parse(&input.to_string()).is_err());
    }
    #[test]
    fn capabilities_and_context_are_explicit() {
        let limited = Capabilities {
            creates: true,
            archives: false,
            complete_history: false,
        };
        assert!(limited.require(&[EventKind::Archive], false).is_err());
        assert!(limited.require(&[EventKind::Create], true).is_err());
        assert!(limited.require(&[EventKind::Create], false).is_ok());
        let a = SourceContext::new("c".into(), "ab".into(), 1).unwrap();
        let b = SourceContext::new("bc".into(), "a".into(), 1).unwrap();
        assert_ne!(a.binding(), b.binding());
    }
}
