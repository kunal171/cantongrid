# Design: exercises and lineage

How CantonGrid gets exercise and parentage evidence from PQS, and what that
evidence can prove. This covers milestones M0 (source feasibility), M1 (receipts)
and M2 (lineage), all shipped in 0.4.0. Usage is in
[composition](../guide/composition.md#direct-lineage).

## M0: is PQS tree mode enough?

**Profile:** Canton 3.5.7, Scribe 3.5.4 (schema041), PostgreSQL 17.12, LF 2.1.
Verified on the pinned LocalNet on 2026-11-21.

| Question | Finding |
| --- | --- |
| Selecting exercises | Scribe tree mode (`pipeline.datasource = TransactionTreeStream`) with explicit party filters; no choice-name filter needed |
| Stable paging | One-row `(offset, node_id)` pages and overlapping replay match after exporter restarts |
| Visibility | Four scoped views (operator, A, B, AB) match Ledger Effects witnesses; redaction unverified |
| Descendants | Subtree bounds match Ledger Effects, but hidden exercises prevent a general direct-parent proof; `-1` bounds unsupported |
| Transient contracts | Kept; all three consuming exercises match archive IDs |
| Coverage | Retained window verified, future end rejected; pruning, regression and pre-snapshot history unverified |

**LocalNet run.**
- **Setup:** 3 synthetic parties, exact source users and isolated databases.
- **Transactions:** 5 committed. Unauthorized and rollback controls were rejected.
- **Per view:** creates, exercises and archives were operator 9/5/3, A 7/3/3,
  B 8/3/3 and AB 9/3/3.
- **Matching:** every view matched authorized Ledger Effects at the same
  transaction identities.
  - **Package names:** PQS FQNs use package names, so comparisons must restore the
    concrete package ID.
- **What the views showed:**
  - **Hidden factory:** A and B see the pair's children without seeing the
    nonconsuming factory exercise.
  - **Inherited informees:** both observers see the whole nested consuming
    transaction, including a child whose stakeholders name only A.
  - **Two boundaries:** event witnesses and ACS stakeholders are different
    boundaries.

**Decisions.**
- **PQS is enough for receipts:** tree mode suffices for M1, and no second
  ingestion adapter is justified.
- **No parent guessing:** neither source recovers an exercise hidden from the
  same view. A filtered subtree may prove ancestry without naming the immediate
  parent, so the nearest visible exercise is never promoted to "parent".
- **Not proof:** timestamps, application references and choice names never prove
  ancestry.
- **Archive ownership:** `archives()` keeps ownership of lifecycle effects, and
  exercises are separate evidence.

Reproduction is in `examples/exercises`. Run `scripts/dev.py check exercises` for
the read-only catalog probe.

## M1: exercise receipts

- **Opt-in:** `"exercise_source": "pqs041_ledger_effects"` changes the durable
  binding, so use a new generation and a separately prepared source database.
  Legacy configurations are unchanged.
- **History:** the initial snapshot stays partial. Exercises begin after it, and
  no receipts are invented for active contracts.
- **Pages:** one complete transaction per forward page, with at most 64 lifecycle
  and exercise records and 1 MiB. Oversized input is rejected without moving the
  checkpoint.
- **Receipt contents:**
  - **Identity:** offset, node ID and update ID.
  - **Exercise:** the exact template and package, the contract, the choice, the
    consuming flag, and the argument and result.
  - **Parties:** controllers and witnesses.
  - **Time and bounds:** the time and the descendant bound. Unknown bounds stay
    unknown.
- **No state changes:** exercises never change contract or model state. Evidence
  may target a contract without an indexed create, and that never repairs state.
- **Storage:**
  - **Migration 009:** private receipts, delivery evidence and an internal outbox,
    committed in the writer transaction.
  - **Keys:** `(stream, offset, node_id)`.
  - **Replays:** an identical replay does nothing. Changed content under the same
    identity rejects the delivery.
- **Not supported:** package handoff for receipt streams, interface choices,
  redaction (which stops the page) and reassignments.
- **Testing:** the real worker test gave 4 receipts, 3 consuming exercises, 2
  unindexed targets and 1 archive, and no extra receipt after an idle restart.

## M2: direct lineage

- **Opt-in:** `"lineage_source": "pqs041_complete_subtrees"` with exercise
  ingestion, as a new generation.
  - **Creation metadata:** forward creation metadata is kept, including
    transient and divulged nodes, without adding those to contract state.
- **When an edge is direct:**
  - **Interval:** the containing exercise's whole numeric node interval must be
    present, unique and consistently nested, with known bounds.
  - **Nodes:** each node is a supported create or exercise in the same scoped
    transaction.
  - **Choice of parent:** the innermost containing exercise is picked only after
    those checks.
  - **No edge:** gaps, unknown bounds and crossing intervals prove nothing.
  - **Conservative on purpose:** this criterion doesn't claim that exported node
    IDs are always consecutive.
- **Storage (migration 010):**
  - **Tables:** creation and page evidence, edges and an edge outbox.
  - **Limits:** 64 provenance nodes per transaction, and 1 MiB for witness policy.
  - **Visibility:** each edge keeps the witness obligations of its proof nodes (AND
    across nodes, OR within one). Reads also require both endpoint contracts to be
    visible.
- **API:** `/v1/contracts/{id}/lineage` and `contractLineage` give one-hop parents
  or children.
  - **Unavailable roots:** hidden and unknown roots look the same.
  - **Hidden edges:** they leave no trace.
  - **Coverage:** always `partial_recorded_evidence`.

| LocalNet scenario | Operator root edges | A / B / shared |
| --- | ---: | --- |
| Generic pair and nested choices | 3 | 1 / 1 / 1 |
| Outcomia `Protocol_CreateBinary` | 2 | 1 / 1 / 1 |

**What the Outcomia run showed.**
- **Operator:** sees the successor protocol and the draft market.
- **Restricted admins:** see only the protocol successor, because the divulged
  market isn't indexed.
- **Consistency:** both transports agree, and revocation stops access.

Counts are in `examples/exercises/lineage-evidence.json`.
