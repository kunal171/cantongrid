# Working on CantonGrid

- Active task: exact-value reconciliation on feat/reconciliation-policies after
  stable 0.4.0. Offline policies and model preflight are implemented. The user's
  continuation authorizes the documented storage-only indexed-contract slice in
  docs/design/reconciliation.md. Keep full source/caller scope before any reads,
  complete retained indexed boundaries, exact active revisions and bounded work.
  Indexed reads are now owned-PostgreSQL-tested; LocalNet/remote evidence and HTTP
  exposure remain later work. No new dependency, migration, public API, ledger
  write, publication or financial all-clear claim is authorized by this slice.
  Keep focused commits and detailed function/type comments.
- Existing baseline: 0.4.0 promotes alpha.7. Alpha.6 added bounded temporal queries, durable canonical
  delivery and local operational hardening. Release 0.3.0 includes PostgreSQL
  17.12 source/destination compatibility and 261-contract LocalNet staged rebuild
  acceptance. Funded trading remains later work; canonical durable delivery is now implemented.
  Use docs/README.md for scope and docs/verification.md for dated evidence;
  do not claim full Phase 4 or production acceptance.
- Aggregate reads require callers to cover the complete configured source-party
  scope before any group read. Hiding groups from partial-scope callers can reveal
  hidden membership through disappearance; do not replace this guard with row filtering.
  See docs/guide/aggregates.md for the implemented count/sum slice and evidence.
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

- crates.io publication is an explicit operator action. Keep package archives
  self-contained, retain licenses/notices, and run the workspace dry run and
  archive checks. CI must never upload packages on ordinary pushes or PRs.
