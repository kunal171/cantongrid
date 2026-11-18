# Operations

Run one continuous worker per source scope and a separate API process. The CLI
is not a process supervisor. Start LocalNet, PQS and the owned destination before
starting `index`/`serve`. Resume with the same reviewed source configuration.

A committed checkpoint survives restart. Failed pages roll back; continuous
indexing retries every two seconds. `--once` returns after one page or failure.
See [storage behavior](../architecture.md#destination-and-recovery).

## Troubleshooting

| Symptom | Action |
| --- | --- |
| Unsupported command | Check `--help`; `dev`, `doctor`, live `status` and subscriptions are not implemented |
| `init` refuses a directory | Use a new directory; it does not overwrite |
| Runtime JSON fails fixture validation | Use `index`/`serve`; configuration schemas differ |
| Indexing pauses | Check token expiry, exact rights, PQS readiness and destination connectivity |
| API denies a bearer | Check issuer/audience/lifetime, source health, principal status and party subset |
| Old cursor fails | Restart pagination after expiry or grant changes |
| Unexpected empty result | Check exact template/package, caller scope and coverage; do not broaden access |
| Binding/checkpoint conflict | Review source identity/scope/generation; do not force another source's offsets |
| Source gap | Review retention/boundaries; do not skip data or claim complete history |
| Numeric error | Use exact decimal strings, supported operators and declared scale |
| Model binding refuses a replacement | Register/install the new revision and supply the expected old revision with `--replace` |
| Model backfill/work limit | Atomic backfill is capped at 128 rows/8 MiB; staged rebuilds are not available; do not skip contracts |
| Model projection failure | Correct payload/mapping or reviewed schema drift; retry without moving the checkpoint |
| Schema mismatch | Explicitly migrate only the owned destination; never edit applied migration checksums |

Refresh local PQS credentials using the matching `pqs up` helper. Renew the
worker/API source token for its exact Ledger user and restart those processes
with the updated environment. Exporting a variable in another shell does not
update a running process. Key rotation also invalidates old tokens/cursors.

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
prune volumes. Retained history grows; automatic erasure is not implemented.

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
