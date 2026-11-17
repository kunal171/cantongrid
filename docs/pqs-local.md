# Local PQS compatibility fixture

This is development infrastructure for the future Rust indexer. It runs the
Quickstart-pinned Scribe/PQS 3.5.4 against LocalNet Canton 3.5.7. Its separate
PostgreSQL is a source-query store, not CantonGrid's destination database.
No CantonGrid ingestion engine, API server or application model is implemented.

## Start and stop

After LocalNet is ready, run from CantonGrid:

```bash
python3 scripts/localnet-pqs.py up
python3 scripts/localnet-pqs.py status
python3 scripts/localnet-pqs.py stop
```

`up` creates or verifies the dedicated local ledger user
`cantongrid-pqs-app-provider`. It derives the existing App Provider primary party
from the running participant, then requires exactly one CanReadAs grant for that
party. It adds no parties, ActAs rights or administrator rights. If an existing
user differs, it fails without replacing its permissions.

The helper writes a one-hour development credential into owner-only
`.local/pqs/source.conf`. The Scribe image runs as the caller's UID/GID so it can
read that file without making it public. Never copy these public-development-key
credentials or configuration to a shared network. This helper is intentionally
limited to known localhost Ledger API endpoints.

PQS has its own Compose project, named volume and database. PostgreSQL publishes
`127.0.0.1:5441`; PQS health publishes `127.0.0.1:18080`. The source process joins
the LocalNet network to read the participant's gRPC port 3901. Its image is pinned
by digest in `dev/pqs.compose.yaml`. Host PostgreSQL 5432 and LocalNet's database
5440 remain separate.

The fixture selects the one authorized party, all visible templates and package
metadata, exact numeric/string encoding, and start offset 0. A start request is
not proof of complete network history. The source contains existing Splice demo
contracts, not uploaded Outcomia contracts. Readiness is available at `/readyz`;
container creation alone is not readiness.

## Credential and lifecycle limits

`up` renews the token and recreates the source process while retaining the
PostgreSQL volume. It does not implement production OAuth refresh or a daemon
that renews credentials. Restart it when the local token expires. Persisted rows
alone do not prove a healthy authenticated source connection.

`stop` removes this fixture's containers/network and keeps its database volume.
The local ledger user and ignored config remain. There is no automatic destructive
reset, party allocation, DAR upload or transaction submission.

Stop PQS **before** stopping LocalNet because it attaches to the LocalNet network:

```bash
python3 scripts/localnet-pqs.py stop
python3 scripts/localnet.py stop
```

After an interrupted host boot, some upstream services may start before their
database and retain failed initialization state. If `up` does not restore API
readiness, stop in that order, then start LocalNet and PQS again. Keep the named
volumes; do not use global Docker cleanup to solve initialization problems.

## Source and verification boundaries

The fixture owns PQS schema writes in its own database. Research SQL runs under
its local owner inside a read-only transaction; this is not a production database
reader-role configuration or per-caller API authorization implementation.

See the [official PQS guide](https://docs.canton.network/sdks-tools/development-tools/pqs)
and [SQL reference](https://docs.canton.network/appdev/reference/pqs-sql-reference).
Their current examples are not a substitute for checking schema 041's actual
function definitions and behavior. A future Rust adapter must declare a tested
version/capability profile, source coverage and explicit authorization scope.
