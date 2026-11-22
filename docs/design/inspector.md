# Read-only developer inspector (M5)

Status: planned implementation on `feat/developer-inspector`, after alpha.4.

## Supported surfaces

- Authenticated REST/GraphQL transaction inspection over M2 creation/exercise
  evidence and correlated canonical archives. Require independently visible
  indexed contract endpoints plus source-event witness obligations. Unindexed
  transient/divulged targets stay unavailable. Do not reconstruct rejected
  transactions or invent missing events.
- Flat recorded-node order, renumbered **after** visibility. No raw node IDs,
  subtree bounds, controllers, arguments/results, payloads, witness lists, user
  IDs, URLs, SQL or private checkpoint keys. No guessed nesting: callers can use
  M2's independently guarded direct-lineage API for proven edges.
- Visible records only in totals, paging and coverage. Unknown/hidden transactions
  have the same unavailable response. Always report partial recorded evidence;
  never signal a hidden descendant with a per-transaction incompleteness flag.
  Page 1–25/default20; at most 128 visible records, 2 MiB response. Signed cursors
  bind operation, transaction, caller policy and physical plan/generation. Cursor
  positions are visible ordinals, never source-node numbers.
- Authenticated per-contract projection diagnosis: current visible canonical
  input, current bound model revision, sanitized evaluation status and native SQL
  comparison with the maintained row. No values or aggregate groups are returned.
  Archived/filtered inputs should have no current row. Unknown/hidden roots share
  one unavailable response. Verify active bindings and installed schemas in the
  same read-only snapshot; recheck the catalog before delivery.
- Local `inspect evaluate` evaluates an operator-supplied payload against a model
  and emits only model identity/status/error codes. It does not accept log files
  or re-execute ledger commands; failed transactions are not stored by the engine.
- Local `inspect progress` renews current source authorization and health, then
  reads restricted source bounds and owned checkpoint/model/rebuild metadata.
  Global watermarks/counters never become narrow-caller API data. Report source
  and destination boundaries as independent observations, not a measured lag
  duration or proof of complete history. No credentials, private scan keys or raw
  cursor documents in output. Existing rejected operations have no durable failed
  trace: preserve/report this limitation rather than manufacturing one.

## Implementation and compatibility

Use existing migration 010 and canonical model/batch/receipt tables. No new
runtime dependency, database migration or diagnostic write journal. Share API
admission, current grants/source health, signed cursors and final encoding guards.
Evidence export is a JSON response or stdout redirection of the sanitized local
commands, never automatic inclusion of source files or logs.

## Acceptance

Owned PostgreSQL and mock-authority fixtures cover A/B/shared/ungranted/revoked
callers, hidden/unknown equivalence, filtering before ordinal/count/paging,
cross-transaction/cross-operation cursors, source outage/expiry, same-offset
create/exercise/archive reconciliation and no guessed transient endpoints.
Projection controls cover matching/filtered/archived rows, drift, exact numeric
comparison, rollback and binding promotion. Local model diagnostics must not
include a deliberately secret payload, field path or parser/SQL diagnostic.
Progress reads must not advance checkpoints or change model/rebuild rows.
Document fixture and retained-source evidence separately from live LocalNet
acceptance. Add example output and a reproducible guide. Do not publish.
