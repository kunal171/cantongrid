# Exercise source feasibility (M0)

Status: M0 verified on the pinned LocalNet fixture, 2026-11-21.
Baseline: `bd8d0f15f8ac632b4fbfcc0f481ff8a350bf09f9`.
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
| Exercise selection | Exact `exercises` overloads, columns and choice-filter configuration | Pinned Scribe tree mode works with explicit party filters and template wildcard; no separate choice-name filter needed |
| Stable paging | Offset plus stable event/node tie breaker; overlapping pages | One-row `(offset, node_id)` pages and overlapping replay match after exporter restarts |
| Exercise visibility | Actual consuming/nonconsuming witness semantics and redaction | Four scoped views match Ledger Effects witnesses; redaction unsupported/unverified |
| Descendant evidence | Source node identity and valid subtree bounds | Live bounds match Ledger Effects; hidden exercises prevent general direct-parent proof; `-1` remains unsupported |
| Transient contracts | Create/consume within one committed transaction | Relay and temporary Child receipts retained; all three consuming exercises match archive IDs |
| Coverage | Retained offsets, pruning, source regression and filtered nodes | Fixed retained window verified; future end rejected; pruning/regression and pre-snapshot completeness unverified |
| Lifecycle reconciliation | One canonical archive per consuming exercise/ACS delta | Three matching archive identities; propose existing archive ownership, exercise receipts carry evidence only |
| Ledger Effects comparison | Same authorized participant/scope and transaction identities | Operator, A, B and AB match identities/payloads/witnesses/controllers/bounds |

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

## LocalNet results

The approved experiment used three new synthetic parties, exact source users,
isolated databases and restricted SQL readers. Five transactions committed;
unauthorized choice and rollback controls were rejected. Both IDE scripts pass.
See [reproduction](../../examples/exercises/README.md) and
[public evidence](../../examples/exercises/evidence.json).

| Source view | Creates | Exercises | Archives |
| --- | ---: | ---: | ---: |
| Operator | 9 | 5 | 3 |
| A | 7 | 3 | 3 |
| B | 8 | 3 | 3 |
| AB | 9 | 3 | 3 |

Every view matches authorized Ledger Effects at the same transaction identities.
Exercise payload/result, controllers, witnesses and descendant bounds match;
creates also match payload/stakeholders/witnesses. Archive comparison covers
event and contract identities only, not independent archive-witness authority.
PQS FQNs use package names: comparison must restore the concrete `package_id`
before comparing Ledger template IDs. A matching package-name alias is insufficient.

A/B see the pair's children without seeing its nonconsuming factory exercise.
The shared pair exposes one child to AB; it does not expose its operator-only
sibling. Conversely, both observers see all seven nodes of the consuming nested
transaction through inherited informees, including the child whose stakeholder
list names only A. Event witnesses and ACS stakeholders are different boundaries.
The forged child has a matching declared factory reference and no factory event.

PQS rejects a query end after its latest observed offset. Each comparison clips
to its observed boundary and records the unverified tail; a hidden final
transaction need not advance every scoped exporter. Never label this complete
global history. No pruning, redaction, cross-participant or upgrade experiment ran.

Decision: PQS tree mode is sufficient for the M1 internal exercise-receipt slice
on this profile. A second ingestion adapter is not justified by this experiment.
Neither source can recover an exercise hidden from the same authorized view;
general immediate-parent provenance remains unsupported. M0 enables no graph API;
[M2](lineage.md) adds a narrower, opt-in complete-subtree proof profile.

## Proposed M1 contract, pending implementation review

- Scope event keys by source/network/generation plus transaction offset/node ID;
  never expose database-local PKs as portable identities. Confirm replay stability
  in the same participant view before accepting the key.
- Keep committed transaction boundaries intact. Explicitly bound transaction
  size; refuse an oversized transaction without advancing its checkpoint. Test
  pages containing several events at one offset and overlapping restart pages.
- Retain `archives()` as owner of existing lifecycle/model/outbox effects.
  Store exercises as separate evidence, linking consuming receipts by observed
  source identity without applying another archive. The fixture's three matching
  receipts support this choice; filtered/divulged and upgraded cases still need
  implementation controls. Preserve existing `divulged_only` guards.
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

M0's pinned fixture is reproducible, including deliberately restricted views and
explicit unsupported relationships. This is source-feasibility evidence, not
production acceptance or proof of complete history.
M1 then adds internal exercise receipts and replay/storage tests; M2 adds guarded
lineage queries. Joins, temporal queries, inspectors, streams and reconciliation
remain later milestones. Reuse metadata/sync for later impact analysis.
