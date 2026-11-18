# Fixture format

Fixtures are synthetic complete create/archive histories. They do not contact
Canton or authenticate users. See [examples](https://github.com/kunal171/cantongrid/blob/main/examples/fixtures/README.md) and
[commands](guide/quickstart.md).

## Configuration and values

`cantongrid.json` uses `version`, `model_version` and `payload_schema_version` 1,
with adapter `fixture`, explicit source/network/generation, unique parties and a
relative event-file path. Select exact 64-character lowercase package IDs,
module/entity names and both create/archive events with complete history.
Wildcard/duplicate selections, live credentials and model declarations are rejected.

Payloads use tagged Text, Party, Bool, Int64, Numeric, Optional, Enum, List and
Record values. Int64 and Numeric stay canonical strings; Numeric scale is 0–37
and precision is 38 digits. Nesting and unsupported values are checked. This
encoding is not the Ledger API wire format or inferred Daml schema metadata.
The separate [model evaluator](guide/models.md) supports a smaller scalar vocabulary.

## Replay and visibility

The reference engine stages each batch before publishing it. Exact duplicate
redelivery is a no-op. Conflicting event/position identities, contract reuse,
unknown/repeated archives and invalid/out-of-scope evidence fail atomically.
Opaque source positions are ordered by delivery, not lexically or numerically.

Archive history retains creation and archive visibility clauses. Reads require
caller/source intersection for every clause and a valid source lease. Fixture
`--party` flags simulate this policy; they are not an authentication API.

The fixture engine requires history from origin and rejects orphan archives.
It does not implement snapshots, exercises or reassignments. Its in-memory and
bounded PostgreSQL replay paths are distinct from the live worker; see
[architecture](architecture.md#destination-and-recovery).
