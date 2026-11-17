# Rust fixture milestone

Work in progress: strict project configuration is implemented in `core::config`.
The runner and fixture commands follow in this milestone. This is a synthetic,
credential-free boundary; it does not assert live PQS or Ledger API compatibility.

Project JSON uses explicit `version`, `model_version` and `payload_schema_version`
(all currently 1). A source declares the `fixture` adapter, stable source `id`,
`network`, positive authorization `generation`, unique permitted `parties` and
fixture path. Selections use exact package ID/module/entity identifiers, both
create/archive event kinds and complete history. Partial histories, unknown
fields, live adapters and processing operators fail validation. No credentials
belong in fixture configuration. Paths will resolve relative to the project file.

Source capabilities describe supported lifecycle/history operations; they are not
inferred from a successful empty response. The fixture profile supports create,
archive and complete synthetic history only. Source/network/generation bind
visibility and progress. Versions must match before ingestion; a different model
requires a separate rebuild, never reuse of old state.

Serde and serde_json are the first dependencies, locked in Cargo.lock. Typed
configuration rejects unknown fields. Diagnostics deliberately suppress parser
messages that could include field names, credentials or payload values. Runtime
and API authorization still require trusted grant providers; fixture party names
are test inputs only.
