# Failure classification and contribution candidates

For each observed failure: record exact versions, reproduce it, distinguish our
configuration/processing from source behavior, and compare current upstream code
and existing reports. A small standalone reproduction should precede an upstream
issue or patch. Keep credentials, private party IDs and application data out of
public evidence. This workflow does not automatically publish anything.

## Findings — 2026-11-17

| Observation | Classification | Action / evidence |
| --- | --- | --- |
| Invalid party hint derived from a hyphenated Docker network name | Our local configuration | Supply the explicit valid hint; not an upstream defect claim. |
| Owner-only config unreadable by image's default nonroot UID | Our host/container permission mismatch | Run the owned fixture as the caller UID; retain private file permissions. |
| Logging level Warn rejected | Our version-specific configuration | Use the installed version's Warning enum value. |
| set_latest(NULL) returns NULL rather than the resolved offset | Tested older-profile/API documentation difference | Pin resolved offsets explicitly. Not filed as a current upstream defect without checking applicable version semantics. |
| Initial migration skips types because another schema has __event_type | Confirmed upstream SQL defect | [Independent PostgreSQL reproduction and local issue draft](../repros/pqs-schema-type-collision/README.md); packaged and current-source controls tested. |
| Cached B-only row remains after revoking B from the ledger reader | Retained source data; our serving-layer responsibility | Revalidate current authorization and deny stale scope access. This is not evidence that database retention itself violates PQS's contract. |

## Confirmed migration candidate

The unmodified upstream initial migration fails in a fresh isolated PostgreSQL
container when an unrelated schema contains the same type name. No CantonGrid
processing, connection credentials or second PQS writer is needed. A scoped type
lookup passes the same automatic control cases. The current-source file is
identical to the packaged migration used by the failing version.

The local report distinguishes this from related issue 203 and records that no
full current-executable test or Scala suite was run. Do not present the small
in-memory correction as a merge-ready migration edit: deployed Flyway checksums
and the upstream immutable-migration policy need a reviewed solution.

The initial investigation kept its report local. On explicit user instruction,
[upstream issue #240](https://github.com/digital-asset/participant-query-store/issues/240),
[draft fix PR #241](https://github.com/digital-asset/participant-query-store/pull/241)
and [CantonGrid tracking issue #1](https://github.com/kunal171/cantongrid/issues/1)
were opened and cross-linked. The fix uses the callback rather than editing V001;
all versioned migration checksums remain unchanged. Full Mill verification remains
pending; no CantonGrid PR or merge was performed.
