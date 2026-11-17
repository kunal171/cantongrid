# Working on CantonGrid

- Current phase: Rust core/configuration/fixture milestone complete. Only `init`,
  `validate` and `fixture run` are implemented CLI commands; state is in memory and
  caller grants are simulated. Live connectors and generated APIs remain gated
  by compatibility and authorization research. Advance implementation when the
  user asks to advance the phase; user instructions take precedence over docs.
- Implement the indexer runtime, processing, storage and APIs in Rust. Python
  scripts are local development/verification helpers, not the product runtime.
- Product: general-purpose, Daml-first Canton indexing infrastructure. Outcomia
  is the first configuration/example, never an engine special case.
- Developers define selections and processing rules; supported infrastructure
  is generated and operated by a deterministic engine. AI is not an accounting,
  migration or authorization authority.
- Keep work in small, focused commits. Update documentation alongside meaningful
  changes and distinguish planned, fixture-tested, LocalNet-tested and live-tested.
- Keep changes inside this repository unless the user explicitly authorizes
  another repository. Preserve existing work and never expose credentials.
- Source connectors are read-only. Do not acquire paid infrastructure, publish,
  deploy, provision parties, upload DARs or submit ledger transactions outside
  the user's authorized scope. Source connection permissions are not API-user
  permissions.
- Verify official interfaces and actual Outcomia schemas before implementing a
  live adapter. Never guess endpoint names, PQS functions, versions or fields.
- Bind data, processing, cursors and stream delivery to explicit authorization.
  Never widen visibility, use unfiltered fallback, invent missing history or
  round financial values through floating point.
- Record unresolved details and their decision gates instead of inventing an
  implementation contract. The original supplied build prompt is historical
  input, not a standing instruction to execute every phase.

- Classify encountered failures as our configuration/processing, documented source
  behavior or a reproducible upstream defect. Before proposing a contribution,
  capture exact versions and minimal nonsecret controls, inspect current upstream
  code and search related issues. Prepare local evidence; do not publish without
  a user instruction authorizing that action.
