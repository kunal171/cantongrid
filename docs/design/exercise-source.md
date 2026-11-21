# Exercise source feasibility (M0)

Status: in progress. Baseline: `bd8d0f15f8ac632b4fbfcc0f481ff8a350bf09f9`.
Scope: the 2026-11-21 technical roadmap's M0 only. This RFC records evidence and
source decisions; it does not introduce canonical exercise events, destination
migrations, graph APIs or Outcomia changes. M1 needs a separate instruction.

## Starting point

CantonGrid 0.4.0-alpha.2 has ten crates, schema041 create/archive ingestion,
independent source/caller grants, private models, staged package upgrades, DAR
metadata/sync and explicit HTTPS transport. Applied destination migrations end at
008. Existing release/test records are dated evidence, not checks rerun here.

The study profile is Canton 3.5.7, Scribe 3.5.4/schema041, PostgreSQL 17.12 and
Daml LF 2.1. Reinspect actual SQL/service versions before treating a source as
compatible. Offline upstream code is evidence of a possible interface, not proof
of running exporter configuration or authorized event coverage.

## Evidence to establish

| Question | Required control | Status |
| --- | --- | --- |
| Exercise selection | Exact `exercises` overloads, columns and choice-filter configuration | Retained schema041 catalog inspected; tree-mode selection still needs pinned live acceptance |
| Stable paging | Offset plus stable event/node tie breaker; overlapping pages | Structured `(offset, node_id)` found; overlap/restart controls pending |
| Exercise visibility | Actual consuming/nonconsuming witness semantics and redaction | Catalog has `witnesses text[]`; populated values/redaction unverified |
| Descendant evidence | Source node identity and valid subtree bounds | Catalog has `last_descendant_node_id`; upstream migration default is `-1`; running values unverified |
| Transient contracts | Create/consume within one committed transaction | Generic IDE-ledger fixture passes; exporter handling pending |
| Coverage | Retained offsets, pruning, source regression and filtered nodes | Pending |
| Lifecycle reconciliation | One canonical archive per consuming exercise/ACS delta | Design decision pending evidence |
| Ledger Effects comparison | Same authorized participant/scope and transaction identities | Pending |

A filtered subtree may prove ancestry without identifying the immediate parent.
Do not elevate the nearest visible exercise to a direct-parent claim. Negative
sentinels, missing nodes and pre-snapshot history remain unsupported/incomplete.
Exercise actors, contract stakeholders and returned witnesses are separate facts.

## Observed source contract (2026-11-21)

The retained PostgreSQL 17.12 schema041 database was inspected through its
existing SELECT-only reader, in a read-only snapshot. Neither Ledger nor exporter
was running. No event rows were read. Reproduce with
`python3 scripts/dev.py check exercises`; private evidence is stored under
`.local/exercise-source/`.

The public overload is `exercises(qname text, from_offset bigint, to_offset bigint)`
returning `SETOF exercise`; defaults use retained/pruned and latest offsets. Its
stored body applies inclusive `between` bounds through offset-to-index helpers.
The composite includes package/template/choice identity, consuming flag, contract
ID, argument/result, effective time, redaction ID, exercise event PK and structured
event ID, transaction index/offset, signatories/observers/controllers, subtree
bound and witnesses. `event_id` contains `offset bigint` and `node_id integer`.
The transaction table has a separate `transaction_id`; it is not an exercise
composite attribute. None of these catalog facts proves populated row semantics.

Function-definition evidence SHA-256:
`5cdf84336a609713d0225227ca6973efe124426a7522f45f23f6af8b3ab0569b`.

Cached upstream PQS checkout `77082b2` selects `getTransactionTrees` for
`TransactionTreeStream`, which requests Ledger Effects. Its default
`TransactionStream` selects ACS delta; template/interface filtering includes
choices in tree mode rather than a separate choice-name filter. Relevant files:
`modules/pipeline/src/com/digitalasset/pqs/pipeline/pipeline/{Config,Pipeline}.scala`,
`modules/zio-daml/src/com/digitalasset/zio/daml/ledgerapi/UpdateService.scala` and
`ContractFilteringSpec.scala`. This checkout is not proof of the pinned Scribe
3.5.4 executable. A network-isolated pinned-image startup printed
`datasource=TransactionStream`; its `--help` argument did not stop startup, so it
failed to connect to a database. It had no credentials, mounts or network access.

The existing CantonGrid fixture profiles omit `pipeline.datasource`; do not
assume their retained databases contain exercises. A separately approved tree-mode
experiment must not repurpose an existing ACS-delta checkpoint or database.

## Proposed M1 decisions, pending live evidence

- Scope event keys by source/network/generation plus transaction offset/node ID;
  never expose database-local PKs as portable identities. Confirm replay stability
  in the same participant view before accepting the key.
- Keep committed transaction boundaries intact. Explicitly bound transaction
  size; refuse an oversized transaction without advancing its checkpoint. Test
  pages containing several events at one offset and overlapping restart pages.
- Retain existing archive ownership until a reviewed reconciliation rule proves
  how consuming exercises and archive rows represent one lifecycle transition.
  Merely observing both must not apply model/outbox effects twice.
- Preserve exercise witnesses separately from contract stakeholders/controllers.
  A visible factory exercise cannot authorize disclosure of every child.
- Treat subtree bounds as ancestry evidence only when identity, scope and
  retained coverage are verified. A missing intermediate node cannot prove the
  immediate parent. Unknown (`-1`) bounds produce no proven relationship.
- Keep source/caller authorization and existing aggregate full-scope guards.
  No graph query, public event type or destination migration is introduced here.

These are review questions, not an implemented adapter or approved M1 contract.

## M0 deliverables

1. A bounded read-only catalog/source probe, with redacted public summary and
   private local evidence under ignored `.local/`. Source queries run under the
   existing restricted reader; no upstream DDL or grants are changed.
2. A generic LF 2.1 fixture covering nonconsuming factory/two children, nested
   consuming exercise, transient creation/consumption and A/B/shared visibility.
   Build and Daml-script validation are offline; deployment/transactions require
   separately authorized, explicit local-only opt-in.
   See [the fixture and controls](../../examples/exercises/README.md).
3. A capability matrix comparing actual PQS output with authorized Ledger Effects:
   identities, ordering, witness scope, descendant structure, pruning and gaps.
4. A proposed M1 source contract choosing authoritative lifecycle ownership,
   transaction-boundary/oversize handling, event keys and visibility evidence.

No parser may guess ancestry from timestamps, application references or choice
names. `declared_reference` is distinct from source-observed ancestry. Hidden
relationships are omitted without IDs/count placeholders; missing evidence for an
authorized object can be reported incomplete without asserting a hidden edge.

## Decision and release gates

Prefer a proven PQS-only path when it preserves required evidence. Introduce a
Ledger Effects source only for demonstrated gaps and a reviewed reconciliation
contract. Do not independently advance two representations of the same lifecycle.
Keep existing create/archive API responses, counts and cursor semantics intact.

M0 is complete only when another developer can reproduce the versioned evidence,
including a deliberately restricted view. If fixture deployment is not authorized
or source visibility is not provable, record the gap and leave acceptance open.
M1 then adds internal exercise receipts and replay/storage tests; M2 adds guarded
lineage queries. Joins, temporal queries, inspectors, streams and reconciliation
remain later milestones. Reuse metadata/sync for later impact analysis.
