# LocalNet contracts runtime

Status: Phase 3 implemented and LocalNet-tested for the profile below. The release
candidate is `0.2.0-alpha.1`; it is not a production or devnet certification. Both
Outcomia and the unrelated Visibility application passed through the same Rust
reader, owned PostgreSQL store and authenticated REST/GraphQL service.

## Run

Build with Rust 1.100 and check free disk space first. Reuse one Cargo target
location; do not leave duplicate SDK/workspace builds or DEBUG logging running.

```bash
cargo build -p cantongrid-cli --locked
python3 scripts/dev.py storage up
cargo run -p cantongrid-cli -- storage init --database-env CANTONGRID_DESTINATION_URL
cargo run -p cantongrid-cli -- index .local/runtime/project.json --once
cargo run -p cantongrid-cli -- index .local/runtime/project.json
cargo run -p cantongrid-cli -- serve .local/runtime/project.json
```

Set the referenced database and token/key environment variables before these
commands. Initialization is explicit and applies only checksummed owned migrations
001 and 002; opening a store never migrates it. `--once` processes at most one
page. Continuous indexing retries failures every two seconds without acknowledging
uncommitted progress. The source token is re-read each cycle; supply a renewed
credential and restart the process when an operator-managed token expires.

Copy [project.json](../examples/runtime/project.json) and
[grants.json](../examples/runtime/grants.json) into an owner-only ignored `.local/`
directory. Replace every synthetic template/party/user with verified existing
metadata. Paths for grants are relative to the project file. The runtime does not
upload packages, allocate parties, change grants or submit transactions.

## Supported source and policies

The supported source is LocalNet Canton 3.5.7, Scribe 3.5.4/schema041, using a
restricted nonowner SELECT-only database reader and a separate Ledger credential
with exactly the configured nonempty CanReadAs set. Numeric-loopback plaintext
transport and the default identity provider are intentional development limits.
Source readiness, current user/rights and SQL schema are separate checks. Source
connection, filter, party, selection or generation changes require explicit review.

The plan fingerprint binds its source/network/generation, exact templates/parties,
source Ledger endpoint/user, SQL host/port/database/reader role and health endpoint.
Password/token rotation preserves that binding. Changing a physical source under
the same generation conflicts with existing progress; it cannot reuse another
participant's offsets. Environment variable names alone are not source identity.

Only exact template projections are supported; interfaces, divulged-only records,
reassignments and generated typed Daml/business models are outside this profile.
`page_size` is 1–8; each raw payload is at most 64 KiB and a destination delivery at
most 1 MiB. Payloads are tagged `raw_json`: their text and source Numeric/Int64
strings remain exact without guessing types or performing floating-point math.

The required policy strings are explicit:

- `snapshot_forward_partial`: capture active contracts at a retained boundary and
  poll subsequent creates/archives. Historical creates before that boundary are
  not invented. A snapshot observation carries its actual original create offset.
- `prior_create_current_grants`: schema041's archive witnesses remain empty; an
  archive is authorized through the previously observed matching creation and
  current grants. Unknown/orphan archives fail, rather than fabricating evidence.
- `retain_while_authorized`: owned state/history remain stored, but serving always
  requires current authorization. Revocation does not erase history or imply an
  archive. Automated retention/erasure is not implemented.

Every page reads one repeatable-read, read-only SQL snapshot, pins its boundary
and orders by offset plus source event PK. The PK is an internal tie breaker.
Restart resumes the acknowledged page; pruning/regression stops with a gap error,
never silent cursor advancement. Controlled Outcomia publication during a paged
snapshot matched all eight expected create/archive contract identities with
single-event pages and worker restarts. This verifies the tested profile, not all
PQS versions or every Canton lifecycle capability.

## API authentication and privacy

Organization bearer tokens use HS256 with exact configured issuer/audience,
`sub`, `exp` and `iat`, no expiry leeway and at most one-hour lifetime. Bring tokens
from a trusted organization issuer. Token claims do not supply party grants;
current party subsets come from the separately managed grants file. Removing or
disabling a principal denies its existing token. Replace grant files atomically
and advance their epoch when changing access.

Application signing and cursor keys must each contain at least 32 bytes and differ
from each other and the source credential. There are no default keys. Supply keys
through the configured environment variables; do not commit them or pass them as
command arguments. Source credentials never authenticate API visitors.

Caller/source checks apply before SQL counts and pages and again before delivery.
Source leases last at most 15 seconds and cannot outlive the source credential.
Source outage or unknown authorization denies serving; instantaneous remote
revocation while disconnected is not claimed. Local grant files are reloaded for
each query and after complete GraphQL operations. No cross-request data cache is
used, and responses carry `Cache-Control: no-store`.

Signed page cursors are opaque API tokens, not encrypted storage. They bind the
principal, grant/configuration fingerprint, query kind and expiry (at most 60
seconds and no later than the bearer). They contain only already-visible row
keys; source party lists and raw internal source cursors are not exposed.

## Query both transports

```http
GET /v1/contracts?limit=25
Authorization: Bearer <organization-token>

GET /v1/events?limit=25&cursor=<nextCursor>
Authorization: Bearer <organization-token>
```

```graphql
query ContractsAndLifecycle($limit: Int, $cursor: String) {
  activeContracts(limit: $limit, cursor: $cursor) {
    items
    visibleTotal
    nextCursor
    provenance
  }
  lifecycleEvents(limit: $limit) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

POST a single GraphQL operation to `/graphql`, with the same bearer header.
Generic `items`/`provenance` use JSON scalars; typed generated fields are Phase 4.
Page sizes are 1–25 (default 20). GraphQL depth is capped at 8 and complexity at 100; request
bodies at 16 KiB, individual result pages at 2 MiB, and concurrent requests at 16.
Mutation, subscriptions and introspection are disabled.

Active records include exact template identity, contract/create-event ID and raw
payload. Lifecycle records expose event/contract/template identity, kind, optional
create payload, exact timestamp strings, original source offset, snapshot status
and visibility-policy provenance. They omit raw witness sets, storage envelopes
and source cursor PKs that could reveal a broader operator scope.

Provenance includes the configured source/network/generation, source-local
watermark, history start, `bootstrapping`/`partial_history`/`unknown` coverage and
observation age. `recently_observed` means observed within 30 seconds, not zero
ledger lag. `stale_or_unknown` remains explicit. Offsets are never global heights
or event counts. API existence/counts/pages remain caller-scoped.

## Reproduce the LocalNet demonstrations

With the existing authorized Visibility source, its isolated reader and the
explicitly initialized destination running:

```bash
python3 scripts/dev.py pqs reader --visibility
CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check pipeline
```

That read-only check uses existing contracts, ephemeral application keys and the
Rust executable. It verifies A=2/B=2/AB=3, exact strings, pagination, worker restart,
local revocation and source outage/recovery while cached SQL data remains present.
It restores services/grants and stops its API child afterward.

The Outcomia example requires separate permission because it uploads an existing
DAR and submits a local test lifecycle. The maintainer approved that local-only
setup in this session. The explicit opt-in command is:

```bash
CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check outcomia-pipeline \
  --dar ../outcomia-canton/contracts/main/.daml/dist/outcomia-markets-0.1.0.dar \
  --allow-ledger-writes
```

It reuses the owned test parties/writer, creates an isolated protocol and a
Draft/Open/Closed prediction-market lifecycle, and checks REST/GraphQL plus exact
price tick and Int64 terms. It submits package-name identifiers with the exact
DAR package ID preference, as required by the tested command API. The Rust
selection still pins the exact creation package/module/entity. No asset holdings,
collateral transfers, orders, trades or resolutions are created. Repeated explicit
runs retain their isolated LocalNet test records; they never reset application
volumes or modify the sibling Outcomia repository. Reports/configuration stay
owner-only under ignored `.local/`.

Devnet, production, remote/TLS transport, generated models and durable streams
remain separate future acceptance. See [Phase 3 verification](phase3-verification.md).
