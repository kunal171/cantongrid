# Outcomia compatibility check

Verified locally on 2026-11-17 against the App Provider participant, Canton 3.5.7.
The initial check below was package validation only. After separate maintainer authorization, a local-only Protocol/Draft/Open/Closed lifecycle also passed the full Rust/PQS/PostgreSQL/REST/GraphQL path. See [Phase 3 acceptance](phase3-verification.md). No funded assets, trades, devnet or production deployment was performed.

## Artifact evidence

The existing sibling Outcomia DAR passed `POST /v2/dars/validate`:

- File: `contracts/main/.daml/dist/outcomia-markets-0.1.0.dar` in outcomia-canton.
- SHA-256: `eef7340eba83f4f2af4865258c66c6461876bacb05b8e4dbc6c7ca74459f6f0e`.
- Main package ID: `73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a`.
- The source project declares SDK 3.7.1 and LF target 2.1. The validated DAR was
  already built; this check does not establish that it matches every current source file.

Reproduce from CantonGrid after LocalNet is ready:

```bash
python3 scripts/dev.py validate-dar \
  ../outcomia-canton/contracts/main/.daml/dist/outcomia-markets-0.1.0.dar
```

The helper reports the exact artifact hash and fails on rejection. It only calls
validation; it does not upload, vet packages, allocate parties or submit commands.
The main package remained absent from the participant's package inventory after
validation. Changing the DAR requires a new validation result and hash.

The [official JSON API specification](https://docs.canton.network/openapi/json-ledger-api/openapi.yaml)
describes this validation endpoint as having no persistent ledger or package effect.
The successful call verifies that endpoint on this particular LocalNet version.

## Initial source review for the example

The actual Canton source defines `Outcomia.Binary:Protocol` and
`Outcomia.Binary:BinaryMarket`. Protocol revisions carry the operator, protocol
name, admin list, market counter and fee settings. BinaryMarket includes its
operator, creator, protocol name, market ID, terms, timestamps and lifecycle state.
Changes such as opening a market consume a revision and create a successor.

Example models must preserve contract IDs and explicitly define revision/domain
identity. Operator/name alone must not be assumed globally unique across protocol
roots. `Closed` stops trading; it does not prove final resolution or settlement.
Custody, orders, positions and resolution use separate templates. A market record
must not be interpreted as proof of collateral or a user's balance.

This is a source review, not a verified metadata extraction pipeline. Do not copy
these domain rules into the generic Rust engine. Typed bindings, actual authorized
contract reads, complete lifecycle fixtures and an unrelated Daml example are
still required. No Outcomia contracts have been indexed in the PQS fixture yet.


## Authorized LocalNet lifecycle update

The same DAR hash/main package above was uploaded after explicit approval. The
controlled test reused owned parties and writer, captured a paged snapshot, then
submitted Protocol_CreateBinary, Binary_Open and Binary_Close. Eight expected
create/archive contract identities matched the index through single-event pages
and worker restarts. Price tick `0.0000000001` and quantity lot
`9223372036854775807` remained exact strings. Both REST and GraphQL succeeded.
No Outcomia-specific Rust branches or sibling repository changes were required.
The helper and explicit opt-in command are documented in [the runtime guide](live-runtime.md).


## Live profile added in 0.2.0-alpha.1

The fixture reference retains its bounded full-journal replay. Live indexing uses
SQL-native row lifecycle checks and a guarded expected checkpoint, so continuous
operation does not hit the fixture's 256-batch/16-MiB journal replay cap. Deliveries
are bounded; state, events, source cursor and outbox still share one transaction.
A stale concurrent worker cannot rewind progress, failed writes roll back the
whole page, and ambiguous commit acknowledgement is resolved by reopening the
acknowledged tail. Journal/history disk usage grows with retained data; automatic
retention/erasure is not implemented.

New immutable owned migration 002 adds observation timestamps and its own checksum
record. Run explicit initialization to migrate an existing owned destination;
opening alone never migrates. No upstream source schema is modified. Read pages
expose observation age and first/complete source boundaries to the authenticated
transports. See [runtime configuration and limits](live-runtime.md).


## Delivered LocalNet subset

The live partial-history policy, current source/caller checks, forward paging and
both API transports are now integrated. Controlled Outcomia publication during a
paged snapshot matched all eight expected lifecycle contract identities, including
same-offset events and restart. Original event offsets are retained separately
from delivery cursors; raw operator witness sets/PKs are not public API fields.
Actual Outcomia and unrelated Visibility both passed the same engine. Earlier
probe-only caveats describe their historical scope; current acceptance and limits
are authoritative in [Phase 3 verification](phase3-verification.md) and
[the runtime guide](live-runtime.md). Full historical completeness, interface/
reassignment support, generated type normalization and remote source integration
remain unsupported; schema041 availability is never a complete global ledger.

The [live runtime](live-runtime.md) now integrates this checker into worker writes
and both query transports; [acceptance evidence](phase3-verification.md) records
its tested scope separately from this standalone probe.


## Phase 3 LocalNet resolution

The earlier gate inventory records the investigation baseline. The supported
schema041 template-only/raw-payload profile is now delivered: explicit prior-create
archive dependencies, current source grants, physical source bindings, bounded
same-offset paging, SQL-native owned state/checkpoint/outbox, independent caller
JWT/grants, scoped cursors and REST/GraphQL. Controlled actual Outcomia and unrelated
Visibility demonstrations passed without production application branches.
See [the acceptance report](phase3-verification.md) for evidence and exact limits.
Interface/reassignment handling, complete history, generated typed metadata and
remote source integration remain open rather than inferred from these results.
