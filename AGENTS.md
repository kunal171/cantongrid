# Working on CantonGrid

- Current phase: Phase 3 has started with bounded owned PostgreSQL fixture ingestion,
  recovery/outbox atomicity and a read-only PQS schema inspector. Live contract
  ingestion, current source/caller verification and APIs remain gated. Fixture
  query party flags simulate grants and must never be used as production identity.
  Advance implementation when the user asks; user instructions take precedence.
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
- Keep development container logs rotated (10m per file, 3 files) and default
  application logging to INFO or quieter. Apply this to new services too; run
  `python3 scripts/dev.py check resources` when changing Compose files. Check
  free disk space before large builds and avoid duplicate Cargo target trees.
  Recover space through owned build tools or container recreation, preserving
  database volumes; never delete Docker-managed log files or globally prune data.
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
