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
| Exercise selection | Exact `exercises` overloads, columns and choice-filter configuration | Pending pinned database/exporter inspection |
| Stable paging | Offset plus stable event/node tie breaker; overlapping pages | Pending |
| Exercise visibility | Actual consuming/nonconsuming witness semantics and redaction | Pending; stakeholder/controller lists are insufficient |
| Descendant evidence | Source node identity and valid subtree bounds | Cached upstream V029 adds a bound with default `-1`; running data unverified |
| Transient contracts | Create/consume within one committed transaction | Pending controlled fixture |
| Coverage | Retained offsets, pruning, source regression and filtered nodes | Pending |
| Lifecycle reconciliation | One canonical archive per consuming exercise/ACS delta | Design decision pending evidence |
| Ledger Effects comparison | Same authorized participant/scope and transaction identities | Pending |

A filtered subtree may prove ancestry without identifying the immediate parent.
Do not elevate the nearest visible exercise to a direct-parent claim. Negative
sentinels, missing nodes and pre-snapshot history remain unsupported/incomplete.
Exercise actors, contract stakeholders and returned witnesses are separate facts.

## M0 deliverables

1. A bounded read-only catalog/source probe, with redacted public summary and
   private local evidence under ignored `.local/`. Source queries run under the
   existing restricted reader; no upstream DDL or grants are changed.
2. A generic LF 2.1 fixture covering nonconsuming factory/two children, nested
   consuming exercise, transient creation/consumption and A/B/shared visibility.
   Build and Daml-script validation are offline; deployment/transactions require
   separately authorized, explicit local-only opt-in.
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
