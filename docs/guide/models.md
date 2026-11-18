# Build a declarative model

Models describe which fields to extract and how to calculate derived values.
The tools validate definitions, preview a local payload, generate SQL plans and
register immutable definitions, install their tables and bind them to an indexed
source. Bindings backfill known active contracts and maintain rows during live
ingestion. The [PoC](poc.md), available in 0.3.0-alpha.2, uses generated TypeScript and
GraphQL schema output plus authenticated model queries.

## Run the complete Visibility example

```bash
cantongrid models validate examples/models/visibility.json
cantongrid models preview examples/models/visibility.json records examples/models/visibility-payload.json
cantongrid models plan examples/models/visibility.json
```

The checked-in payload is synthetic:

```json
{
  "label": "example-record",
  "owner": "A",
  "amount": "12345678901234567890.1234567891",
  "sequence": "9223372036854775807",
  "note": null,
  "state": "Ready"
}
```

Preview returns `matched: true` and explicit typed values. For example:

```json
{
  "amount": {
    "type": "numeric",
    "value": {
      "scale": 10,
      "decimal": "12345678901234567890.1234567891"
    }
  },
  "sequence": {"type": "int64", "value": "9223372036854775807"},
  "note": {"type": "optional", "value": null}
}
```

This is a fragment of `values`, not the complete preview response. Exact numbers
stay strings. Changing state to `Archived` returns `matched: false` and
`values: null`. A missing required field remains an error even when the filter
would otherwise reject the row; filters do not hide malformed input.

## Write your first definition

Save this complete document as `.local/simple-model.json`. Its package ID is a
synthetic placeholder suitable only for offline preview; replace it with verified
package metadata when describing a real application.

```json
{
  "version": 1,
  "models": [
    {
      "name": "quotes",
      "revision": 1,
      "template": {
        "package_id": "0000000000000000000000000000000000000000000000000000000000000000",
        "module": "Example",
        "entity": "Quote"
      },
      "fields": {
        "price": {
          "op": "field",
          "path": ["terms", "price"],
          "as": {"type": "numeric", "scale": 2}
        },
        "doublePrice": {
          "op": "multiply",
          "left": {
            "op": "field",
            "path": ["terms", "price"],
            "as": {"type": "numeric", "scale": 2}
          },
          "right": {
            "op": "literal",
            "value": {"type": "int64", "value": "2"}
          }
        },
        "note": {
          "op": "field",
          "path": ["note"],
          "as": {"type": "optional", "item": {"type": "text"}}
        }
      }
    }
  ]
}
```

Save a matching `.local/simple-payload.json`:

```json
{"terms": {"price": "1.2"}, "note": null}
```

```bash
cantongrid models validate .local/simple-model.json
cantongrid models preview .local/simple-model.json quotes .local/simple-payload.json
cantongrid models plan .local/simple-model.json
```

The result has `price = 1.20`, `doublePrice = 2.40` and an absent optional note.
`1.234` is rejected at scale 2; it is never rounded. Paths traverse objects only,
not arrays. An expression reads input fields, not another projected output field.

## Types, filters and limits

| Type/operator | Input or behavior |
| --- | --- |
| `text`, `party`, `bool` | JSON string, validated party string, JSON boolean respectively |
| `int64` | Canonical decimal string within signed 64-bit range |
| `numeric` | Decimal string, declared scale 0–37, precision up to 38 digits |
| `optional` | One scalar layer; missing/null becomes None |
| `add`, `subtract` | Int64 pairs or Numeric pairs with identical scale |
| `multiply` | Int64×Int64 or Numeric×Int64 in either order |
| `literal` | Explicit typed scalar value |

An optional equality filter compares one projected field with an exactly typed
literal. The Visibility model uses:

```json
{
  "field": "state",
  "equals": {"type": "text", "value": "Ready"}
}
```

This is the value of a model's `filter` property, not a standalone model document.
Filters never widen input visibility. Numeric×Numeric, optional arithmetic,
nested optionals, joins, aggregates, arbitrary SQL and executable plugins are
not supported. Arithmetic overflow fails the evaluation rather than saturating.

Definitions are strict JSON: no duplicate/unknown keys or floating-point tokens.
A document is at most 64 KiB with 1–16 models. Each model has 1–32 fields, at most
256 expression nodes and depth 16; paths have at most 16 segments. Payloads are
at most 64 KiB; serialized projections at most 1 MiB. Text containing NUL is rejected for PostgreSQL
compatibility, including optional text, literals and filter values.

## Register a revision

With the [owned PostgreSQL fixture](quickstart.md#persist-in-postgresql) running:

```bash
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid models register examples/models/visibility.json --namespace demo --database-env CANTONGRID_DESTINATION_URL
cantongrid models inspect demo records 1 --database-env CANTONGRID_DESTINATION_URL
```

Initialization explicitly applies migrations through 005; older destinations
fail to open until upgraded. Migrations 001–004 are unchanged. Registration writes the
whole catalog atomically. Repeating it returns `inserted: 0` and `unchanged: 1`.
Changing an existing namespace/name/revision fails; use a new revision. Inspection
reports registration and fingerprint, without printing stored literals.

Namespaces are operator project labels (up to 64 ASCII characters), not source
identities or permission scopes. Commands require the owned database role, not
Ledger credentials. Registration does not change source checkpoints, install
projection tables or expose a new API. No update/delete registry command exists.

## Understand the SQL plan

`models plan` prints a canonical fingerprint, revision and deterministic table
DDL with `applied: false`. Numeric columns use `numeric(38, scale)`; Int64 uses
`bigint`. Optional fields are nullable. Rows link to a canonical parent contract.
Output names are validated ASCII identifiers, at most 48 characters, with system
names reserved. Table names derive from canonical definition fingerprints.

`plan` remains a dry run. Use the installer below instead of applying SQL manually.

## Install an empty projection table

After registration:

```bash
cantongrid models install demo records 1 --database-env CANTONGRID_DESTINATION_URL
```

The first install reports `created: true`, `linked: true`, `activated: false`.
Repeat it to verify the table and return `created: false`, `linked: false`.
If commit acknowledgement is lost, reconnect and repeat the same command.
Identical definitions across namespaces may share one verified physical table;
namespace labels do not grant data access.

DDL, installation metadata and the revision link commit together. The installer
checks ownership/private grants, native column types/scales/nullability, parent
contract key, indexes and unsupported rules/triggers/inheritance. It rejects
unmanaged existing tables and schema/fingerprint drift; it does not repair them.
It can briefly block writes while checking a table; lock timeouts fail the install.

Installation alone leaves an empty table. Activate it explicitly below.

## Bind and backfill a source

Use a reviewed [runtime configuration](localnet-source.md) whose selected template
matches the installed model. Run at least one successful `index --once` page first:

```bash
cantongrid index .local/runtime.json --once
cantongrid models bind .local/runtime.json demo records 1
cantongrid index .local/runtime.json
```

`bind` verifies current source rights and readiness, then backfills that stream's
known active contracts and activates the revision in one owned transaction.
The result reports `changed`, `input_rows`, `projected_rows` and `source_revision`.
The last value is the destination batch revision, not a Canton offset. A repeat
verifies the installed model and returns `changed: false`.

The worker discovers persisted bindings each cycle. Creation inserts a matched
row; archive removes it. Filters are checked on both paths. Projection errors,
expired source leases or schema drift roll back the entire canonical page,
including history, outbox and checkpoint. Correct the cause before retrying;
processing never skips a rejected event. Model bindings do not change the source
plan or require a new snapshot.

The initial atomic backfill supports at most 128 active contracts of the model's
template and 8 MiB of input/output work. Cleanup is also capped at 128 rows.
Each stream supports at most 16 bindings, including aliases; live pages share an
8 MiB projection work budget. Larger staged rebuilds are not implemented.
Backfill covers the known prefix, including partial snapshots; later pages fill
it forward. It cannot recover missing pre-snapshot history.

## Promote or roll back a revision

Register and install a new revision first, then replace the expected current one:

```bash
cantongrid models register .local/visibility-v2.json --namespace demo --database-env CANTONGRID_DESTINATION_URL
cantongrid models install demo records 2 --database-env CANTONGRID_DESTINATION_URL
cantongrid models bind .local/runtime.json demo records 2 --replace 1
# Explicit rollback also rebuilds from the current canonical prefix:
cantongrid models bind .local/runtime.json demo records 1 --replace 2
```

Revision 2 must keep the same model name and use a runtime-selected template.
Promotion uses the
same source lock as ingestion, so no event can slip between backfill and live
processing. Failure preserves the previous binding and rows. If acknowledgement
is lost, repeat the same command; reaching the requested revision is a verified
no-op even when its `--replace` revision is now stale.

Old rows are removed for this stream only when its last reference to that physical
revision is replaced. Other namespaces and streams retain their rows. Namespaces
remain operator labels, not access grants. No unbind command exists. The [PoC guide](poc.md) covers generated model GraphQL
and client types in 0.3.0-alpha.2.
