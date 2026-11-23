# Operations

Run one continuous worker per source scope and a separate API process. The CLI
is not a process supervisor. Start LocalNet, PQS and the owned destination before
starting `index`/`serve`. Resume with the same reviewed source configuration.

A committed checkpoint survives restart. Failed pages roll back; continuous
indexing retries every two seconds. `--once` returns after one page or failure.
See [storage behavior](../architecture.md#destination-and-recovery).

## Readiness and pressure

```bash
cantongrid ops check-config runtime.json --profile localnet
cantongrid ops preflight runtime.json --profile localnet
cantongrid ops status runtime.json
```

`check-config` is offline policy validation. `preflight` checks current source
rights, selected package availability, readiness, restricted PQS bounds and the
already initialized owned destination. Neither migrates a database. A network
label is operator configuration, not independently verified network identity.
Use [remote setup](remote-source.md) for TestNet/MainNet HTTPS and PQS tunneling.

`ops status` is an operator-local full-source report, not a public metrics route.
It includes ingestion observation age/freshness, queue/subscription/receipt counts,
recorded visible delivery loss and fixed work budgets. Its source/destination
observations are separate, not one atomic snapshot. Keep reports private.
Authenticated `GET /v1/health` returns only readiness and format version; it never
returns party names, payloads, counters or global positions.

A source that is healthy while the worker has stopped is insufficient. Live health
and delivery fail closed once the last ingestion observation exceeds 30 seconds.
Retained historical reads and ACK/cancel operations can continue under current
authority. Consumer activity cannot manufacture worker freshness. Run and monitor
the worker continuously; `index --once` alone is not a live service.

[Delivery](delivery.md) documents retention, gaps, 429 backpressure and consumer
limits. [Recovery](recovery.md) covers private backup/restore and migration rollback.
The [operations example](../../examples/operations/README.md) records reproducible
read-only-ledger LocalNet acceptance and a bounded synthetic database benchmark.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| Unsupported command | Check `--help`; `dev`, `doctor`, top-level `status` are not implemented; use `ops status` |
| `init` refuses a directory | Use a new directory; it does not overwrite |
| Runtime JSON fails fixture validation | Use `index`/`serve`; configuration schemas differ |
| Indexing pauses | Check token expiry, exact rights, PQS readiness and destination connectivity |
| API denies a bearer | Check issuer/audience/lifetime, source health, principal status and party subset |
| Canonical count is slow | Exact totals still scan visibility evidence and are subject to the five-second SQL timeout; payload encoding is limited to the requested page. |
| Old cursor fails | Restart pagination after expiry or grant changes |
| Unexpected empty result | Check exact template/package, caller scope and coverage; do not broaden access |
| Binding/checkpoint conflict | Review source identity/scope/generation; do not force another source's offsets |
| Source gap | Review retention/boundaries; do not skip data or claim complete history |
| Numeric error | Use exact decimal strings, supported operators and declared scale |
| Model binding refuses a replacement | Register/install the new revision and supply the expected old revision with `--replace` |
| Model backfill/work limit | Use [staged rebuilds](models.md#rebuild-a-larger-model-in-stages) above the 128-row atomic cap; lower the step limit for the 8 MiB work budget |
| Rebuild is ready but activation refuses | Check `has_pending`; step again after new ingestion, then retry activation |
| Rebuild reservation blocks another job | Finish cleanup or cancel and step until cancelled; retain the same source plan |
| Model projection failure | Correct payload/mapping or reviewed schema drift; retry without moving the checkpoint |
| API returns 429 / 504 | Admission is capped at 16 and includes body reads; requests have a 30-second deadline. Retry after checking source/destination latency. |
| Destination ACL validation fails | Review schema, table, sequence and column grants; remove unintended access explicitly. Startup does not revoke grants for you. |
| Schema mismatch | Explicitly migrate only the owned destination; never edit applied migration checksums |

Refresh local PQS credentials using the matching `pqs up` helper. Renew the
worker/API source token for its exact Ledger user and restart those processes
with the updated environment. Exporting a variable in another shell does not
update a running process. Key rotation also invalidates old tokens/cursors.

## PostgreSQL support

The source and destination fixtures use digest-pinned **PostgreSQL 17.12** in
new `pqs-data-pg17` and `storage-data-pg17` volumes. PostgreSQL 17 is
[supported through November 2029](https://www.postgresql.org/support/versioning/).

The former PostgreSQL 14 volumes remain intact. Changing the image alone cannot
upgrade a major-version data directory. Stop the workers/exporters, retain a
private logical backup of each database and its roles, restore into separate
17.x volumes, and compare application rows and privileges before restarting.
Use [PostgreSQL's upgrade procedure](https://www.postgresql.org/docs/17/upgrading.html).
Never mount a 14.x directory into a 17.x server or remove the old volumes before
reviewing the restored data. A fresh `up` creates empty 17.x fixtures; it does
not automatically migrate an existing installation.

The pinned upstream Quickstart manages its own participant database. Its image
is unchanged; the source/destination compatibility checks do not establish a
production upgrade procedure for Canton itself.

## Disk and shutdown

```bash
df -h /
docker system df
python3 scripts/dev.py check resources
python3 scripts/dev.py localnet logs
python3 scripts/dev.py pqs status
python3 scripts/dev.py storage status
```

Container logs are capped at 10 MiB × 3 files, with INFO defaults. Apply changed
limits using the corresponding `up` helper; it recreates containers and retains
volumes. Reuse one Cargo target. Do not truncate Docker-managed logs or globally
prune volumes. Canonical history grows; outbox retention does not erase canonical records.

In November 2026, unrotated DEBUG logs consumed about 176 GiB and prevented
PostgreSQL recovery. Container recreation with rotation freed space without a
volume reset. Keep the guard enabled.

Stop the CLI processes, then infrastructure in this order:

```bash
# Only if the optional Visibility source is running:
python3 scripts/dev.py pqs stop --visibility
python3 scripts/dev.py pqs stop
python3 scripts/dev.py storage stop
python3 scripts/dev.py localnet stop
```

Named database volumes are retained. Record exact versions, command, expected
behavior and a synthetic reproduction when reporting failures. Distinguish our
configuration/processing from source behavior before claiming an upstream bug;
see [the reproduced source defect](../verification.md#source-defect).
Use [private reporting](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md) for vulnerabilities.
