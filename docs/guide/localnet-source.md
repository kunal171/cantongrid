# Configure a LocalNet source

The supported live path reads authorized contracts from PQS and writes to a
separately owned PostgreSQL destination. The verified profile is Canton 3.5.7,
Scribe 3.5.4/schema041, exact templates, and raw payloads. Remote transports,
interfaces and reassignments are not supported by this profile.

## Prepare development infrastructure

With Docker/Compose and Python installed:

```bash
python3 scripts/dev.py localnet prepare
python3 scripts/dev.py localnet up
python3 scripts/dev.py localnet status
python3 scripts/dev.py check ledger
python3 scripts/dev.py pqs up
python3 scripts/dev.py pqs reader
python3 scripts/dev.py check reader
python3 scripts/dev.py check pqs
python3 scripts/dev.py storage up
```

Wait for readiness; retry checks after initialization. LocalNet starts the pinned
Quickstart network and its demo state. `pqs up` creates/verifies a dedicated local
read-only Ledger user and its one-party scope, and writes an expiring development
credential under `.local/pqs/`. `pqs reader` provisions a restricted SELECT-only
SQL role in that isolated source database. These are development setup actions,
not work performed implicitly by the Rust indexer.

This does not upload your application's DAR or create application contracts.
Arrange an authorized package/contract lifecycle separately. An empty application
selection can legitimately produce no records; a failed source check cannot.
For environment details, see [LocalNet](../localnet.md) and
[PQS setup](../pqs-local.md), reading their initial research notes alongside the
current [runtime contract](../live-runtime.md).

## Prepare runtime configuration

```bash
umask 077
mkdir -p .local/my-app
cp examples/runtime/project.json .local/my-app/runtime.json
cp examples/runtime/grants.json .local/my-app/grants.json
chmod 600 .local/my-app/*.json
```

Edit the copies with your existing metadata. The checked-in example is a schema
template containing placeholders, not a working source identity.

| Setting | Supply |
| --- | --- |
| `authorization.endpoint` | Existing numeric-loopback JSON Ledger API, e.g. `http://127.0.0.1:3975` |
| `authorization.user` | Existing active user with exactly the selected CanReadAs scope |
| `authorization.parties` | Exact authorized party identifiers, not aliases or wildcard |
| `authorization.source/network/generation` | Stable source identity and reviewed generation |
| `templates` | Verified exact package ID, module and entity for each selection |
| `source_health` | Corresponding PQS readiness endpoint, normally `http://127.0.0.1:18080/readyz` |
| `api` | Loopback bind, organization issuer/audience, separate key references and grants file |

Retain the required policy values in `examples/runtime/project.json`:

```json
{
  "page_size": 8,
  "history": "snapshot_forward_partial",
  "archive_policy": "prior_create_current_grants",
  "payload": "raw_json",
  "retention": "retain_while_authorized"
}
```

This fragment is explanatory, not a complete configuration. The full source file
also defines authorization, database references, templates and API settings.
Runtime documents reject unknown keys; do not add `models` or fixture fields.
`cantongrid validate` validates fixture projects only. The live commands load and
validate runtime settings and source identity before operating.

## Supply credentials and start ingestion

```bash
export CANTONGRID_PQS_URL='host=127.0.0.1 port=5441 user=cantongrid_pqs_reader password=local-pqs-reader dbname=pqs'
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
read -r -s -p 'Existing source bearer token: ' CANTONGRID_SOURCE_TOKEN
export CANTONGRID_SOURCE_TOKEN
printf '\n'
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid source inspect-pqs --database-env CANTONGRID_PQS_URL
```

Use the source credential provisioned for your configured read-only Ledger user.
It must match the selected party scope; an unrelated administrator token is not
a substitute. Do not paste it into committed JSON or command-line arguments.
The defaults above apply only to the disposable local database fixtures.

Complete [API key and caller-grant configuration](authentication.md) before serving.
Then index in one terminal and serve in another with the same required environment:

```bash
# Process at most one bounded page and exit; this does not promise full catch-up.
cantongrid index .local/my-app/runtime.json --once
# Keep processing subsequent pages, including snapshot bootstrap and updates.
cantongrid index .local/my-app/runtime.json
```

```bash
# Separate terminal with the same source/database/API environment configured.
cantongrid serve .local/my-app/runtime.json
```

Keep continuous ingestion running while querying. Initial snapshot pages have
`bootstrapping` coverage; completed snapshot-forward ingestion is `partial_history`.
Contracts that were created and archived before the captured boundary are not
reconstructed. Source offsets are local positions, not global Canton heights.

Source and destination identities are bound to progress. Do not change a physical
source, selected parties or templates and expect old checkpoints to remain valid.
Review scope/generation changes explicitly. The engine never drops a filter to
make ingestion succeed.
