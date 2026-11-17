# Operations and troubleshooting

The current deployment profile is LocalNet-only. Run one continuous worker per
source scope and a separate API process. PostgreSQL guards expected checkpoints
against stale competing writes, but the CLI is not a process supervisor or a
distributed leader-election service.

## Start, resume and stop

1. Start LocalNet, then PQS, then the owned destination.
2. Verify readiness, current source rights and the restricted SQL reader.
3. Apply owned migrations explicitly with `storage init` when required.
4. Start `cantongrid index <runtime.json>` and `cantongrid serve <runtime.json>`.
5. Stop the CLI processes with your terminal/process manager when finished.

Committed progress survives a process restart. State, lifecycle events, checkpoint
and outbox publish in the same transaction. A failed page does not acknowledge
progress. Continuous indexing retries every two seconds; `--once` returns failure
and exits instead of retrying indefinitely. It processes at most one page, not an
entire historical catch-up.

To stop development infrastructure while preserving database volumes:

```bash
python3 scripts/dev.py pqs stop --visibility
python3 scripts/dev.py pqs stop
python3 scripts/dev.py storage stop
python3 scripts/dev.py localnet stop
```

Use the first command only if the separate Visibility PQS experiment is running.
Stop PQS before LocalNet because it attaches to the LocalNet network.

## Diagnose common failures

| Symptom | Check and action |
| --- | --- |
| Unsupported command | Run `cantongrid --help`; `dev`, `doctor`, live `status` and subscription commands are not implemented |
| `init` refuses a directory | Choose a new directory; it never overwrites a project |
| Fixture validation fails on runtime JSON | Use `index`/`serve` for runtime configuration; fixture and runtime schemas differ |
| Indexing pauses | Check source token expiry, exact read rights, PQS readiness and destination connectivity; retained SQL alone is insufficient |
| API denies an existing token | Check issuer/audience/lifetime, source availability, principal enabled status and permitted party subset |
| Pagination fails after grant changes | Restart pagination with the current principal/token; old cursors are intentionally invalid |
| An empty response surprises you | Check selected template/package and current grants, then coverage; never broaden access as a fallback |
| Source binding/checkpoint conflict | Review physical source, selected scope and generation; do not force-reuse another source's offsets |
| Source history gap | Stop and review retention/boundaries; do not advance past missing data or claim complete history |
| Numeric/model error | Use decimal strings, exact scale and supported arithmetic; inspect locally without publishing payloads |
| Destination schema mismatch | Apply reviewed migrations only to the owned destination; never edit a recorded migration checksum |

Refresh expiring local PQS credentials with the matching `pqs up` helper and renew
the runtime's source bearer for its exact Ledger user. Exporting a new variable in
another shell does not update a running process's environment: restart worker/API
processes with the renewed value. Use separate signing and cursor keys of at least
32 bytes. Rotate keys through your operator workflow, then restart affected
processes; do not expect old cursors/tokens to survive a key change.

## Check disk and logs

```bash
df -h /
docker system df
python3 scripts/dev.py check resources
python3 scripts/dev.py localnet status
python3 scripts/dev.py localnet logs
python3 scripts/dev.py pqs status
python3 scripts/dev.py storage status
```

Development containers cap JSON logs at 10 MiB × 3 files per container and use INFO
or quieter defaults. Recreate changed containers with the corresponding `up`
helper to apply logging changes, preserving named volumes. Reuse one Cargo target
directory. Never truncate Docker-managed log files or globally prune volumes.
Retained journal/history also grows; automated data retention/erasure is not
implemented yet.

## Report a problem

Record the CantonGrid version/commit, exact nonsecret source versions, failed
command, expected behavior and a synthetic reproduction. Classify failures as
our configuration/processing, documented source behavior or a reproducible
upstream defect. See [upstream triage](../upstream-triage.md). Share private security
reports through the repository's SECURITY.md process, not a public payload dump.
