# Persist a fixture in PostgreSQL

This exercise adds durable storage without a Canton node. Install Docker Engine,
the Docker Compose plugin with `up --wait` support, and Python 3.11+. Complete the
[CLI build](quickstart.md) first.

## Initialize the owned destination

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
```

This is a separate disposable development database. Its published password is a
fixture default. Port 5442 is distinct from LocalNet storage and the PQS source.
`storage init` explicitly applies checksummed CantonGrid migrations; opening or
indexing a store does not migrate it. Never point this command at upstream PQS.

## Index, inspect and query

```bash
cantongrid fixture index examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture status examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture query examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL --party A
cantongrid fixture query examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL --party A --party B
```

`index` commits deliveries to PostgreSQL. `status` reports operator state;
`query` reconstructs the reference engine from the stored journal and applies
the simulated caller grants. The original `events.json` is not needed to query
the already indexed journal. Status/operator totals are not API-user totals.

Run the first `fixture index` command again. It should report zero newly applied
batches and preserve state/outbox counts. A checkpoint acknowledges only committed
work. This fixture profile uses bounded journal replay; the live worker uses the
separate SQL-native lifecycle path described in the [runtime contract](../live-runtime.md).

## Stop without losing data

```bash
python3 scripts/dev.py storage status
python3 scripts/dev.py storage stop
```

Stop retains the named database volume. A later `storage up` reuses it. Keep the
volume if you want to verify restart recovery. Do not use a global Docker prune
or remove volumes as a normal troubleshooting step.

Next: [configure LocalNet ingestion](localnet-source.md).
