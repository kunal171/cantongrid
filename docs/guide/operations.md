# Operations

**Processes.**
- **Layout:** run one continuous `index` worker per source scope, plus a separate
  `serve` process. The CLI doesn't supervise processes.
- **Checkpoints:** a committed checkpoint survives a restart. A failed page rolls
  back, and the worker retries every 2 s.

## Readiness

```bash
cantongrid ops check-config runtime.json --profile localnet   # offline policy check
cantongrid ops preflight    runtime.json --profile localnet   # live rights, packages, readiness, PQS bounds, destination
cantongrid ops status       runtime.json                      # private operator report
```

- **Migrations:** none of these commands migrate anything.
- **Network labels:** a network label is configuration, not a verified identity.
- **`ops status`:** reports observation age, queue, subscription and receipt
  counts, delivery loss and budgets. Keep it private.
- **`GET /v1/health`:** authenticated, and returns only readiness and format.
- **Worker freshness:** live health and delivery fail closed once the last
  ingestion observation is more than 30 s old.
  - **Not enough:** a healthy source with a stopped worker doesn't count.
  - **Monitoring:** watch the worker. `index --once` isn't a service.

## Remote sources

There is no remote acceptance yet. These steps prepare and check a source; they
don't prove TestNet or production readiness.

**From your node operator, you need:**
- **Ledger:** a JSON Ledger API HTTPS root, the issuer, the identity-provider ID,
  and a dedicated user with exactly the selected **CanReadAs** rights.
  - **Rejected:** admin, CanActAs, wildcards and extra rights.
- **PQS:** the matching PQS/Scribe schema041 export, its readiness URL and a
  separate SELECT-only role. Confirm the party scope, the retained history and
  the JSON encoding.
- **Contracts:** exact package hashes, template names and party IDs. Check them
  with `cantongrid metadata inspect`.
- **Network path:** an approved SSH path to the PQS database. Clients accept only
  numeric-loopback PostgreSQL, so direct remote TLS isn't supported.

```bash
ssh -N -L 127.0.0.1:5544:127.0.0.1:5432 -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 operator@your-node
umask 077 && mkdir -p .local/testnet
cp examples/operations/runtime.testnet.json .local/testnet/runtime.json
cp examples/operations/grants.example.json  .local/testnet/grants.json
cantongrid ops check-config .local/testnet/runtime.json --profile testnet
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL     # owned destination only, after a backup
cantongrid ops preflight .local/testnet/runtime.json --profile testnet
```

**Configuration.**
- **Settings:** keep `network: "testnet"` and `transport: "remote_https"`, and set
  `expected_issuer` and the identity-provider ID.
- **HTTPS:** must be certificate-verified. Redirects, userinfo and insecure TLS
  are rejected.
- **Secrets:** they live in environment variables only: `CANTONGRID_SOURCE_TOKEN`,
  `CANTONGRID_PQS_URL`, `CANTONGRID_DESTINATION_URL`, `CANTONGRID_API_KEY` and
  `CANTONGRID_CURSOR_KEY`.
- **Network identity:** `actualNetworkVerified: false` is intentional. A working
  endpoint doesn't prove which network it is.
- **New participant:** use a fresh source generation. Never reuse LocalNet
  checkpoints.

**Running.**
- **Bind:** keep the API on loopback. Remote exposure needs an approved TLS proxy.
- **Still pending:** scoped reads, isolation, replay and ACK, revocation, outage
  and recovery, using existing authorized contracts.
- **Mainnet:** use `--profile mainnet` only with a separate reviewed config.

## Backup and recovery

Back up before every migration.
- **No downgrades:** an older binary rejects newer migrations. Rolling back means
  restoring the pre-upgrade backup. Never edit migration rows or checksums.
- **Source database:** never migrate or restore into the PQS source.
- **Sensitive dumps:** dumps hold private payloads, parties and consumer progress.
  Keep them owner-only and out of Git.
- **Keys:** back up config, grants and signing keys separately, because dumps
  don't include keys.

```bash
cantongrid storage check --database-env CANTONGRID_DESTINATION_URL
python3 scripts/dev.py recovery verify --source-env CANTONGRID_DESTINATION_URL \
  --restore-env CANTONGRID_RESTORE_URL --output .local/recovery/<new dir>
```

**The helper.**
- **What it does:** dumps a quiesced destination, restores it into a **new empty**
  database owned by a restricted role, and compares the exact rows, sequences and
  ACLs.
- **Requirements:** `pg_dump` must be at least as new as the server.
- **Limits:** a 128 MiB fingerprint cap. It isn't a production backup service.

**To resume on a restored database:**
1. **Restore** the matching config, grants and keys. Change only the destination
   connection.
2. **Check** with `storage check`, then `ops preflight`.
3. **Confirm source history:** the source must still hold the checkpoint
   interval. Start one worker and wait until it's fresh.
4. **Verify reads and consumers:** check scoped reads, and have consumers resume
   without a cursor. Effects after the backup can replay, so deduplicate.

If history was pruned, set up a reviewed baseline. Never jump the checkpoint.

## Inspector

Since 0.4.0. Both inspector endpoints use the normal bearer.

**Transaction records** (needs the lineage profile).
- **Endpoints:** `GET /v1/inspect/transactions/{id}?limit=10`, or
  `inspectTransaction` on `/graphql`.
- **What each record has:** `kind`, `contractId`, `template` and `ordinal`, and
  for exercises also `choice` and `consuming`.
- **Visibility:** only records the caller may see. Hidden transactions return
  404.
- **Coverage:** `partial_recorded_evidence`. No tree is guessed.
- **Limits:** 128 records and 2 MiB.

**Projection consistency.**
- **Endpoints:** `POST /v1/models/{ns}/{name}/inspect` with `{"contract_id"}`,
  or the generated `diagnosticQueryField`.
- **Status:** `matched`, `filtered` or `invalid`.
- **Projection:** `consistent`, `missing`, `unexpected`, `different` or
  `evaluation_failed`.
- **How it compares:** values are compared in PostgreSQL and never returned. Drift
  is reported, never repaired.

**Local tools.**
- **`cantongrid inspect evaluate models.json records input.json`:** evaluates
  local files.
- **`cantongrid inspect progress runtime.json`:** shows source and destination
  boundaries.
  - **Reading it:** `boundaryDifference` is an offset difference, not an event
    count.
  - **Public API:** none.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| Indexing pauses | Check token expiry, exact rights, PQS readiness, destination |
| API denies a bearer | Check issuer, audience, lifetime, principal and party subset |
| Old cursor fails | Restart paging after expiry or grant changes |
| Empty result | Check template, caller scope and coverage; don't widen access |
| Checkpoint conflict | Review source identity and generation; never borrow offsets |
| Source gap | Review retention; never skip data |
| Model bind refuses | Install the new revision and pass `--replace <old>` |
| Backfill limit | Use a [staged rebuild](models.md#staged-rebuilds), or lower `--limit` |
| Rebuild won't activate | Check `has_pending`, step again, retry |
| 429 / 504 | 16 concurrent requests, 30 s deadline; check latency |
| Destination ACL fails | Remove unintended grants yourself (startup doesn't) |
| Schema mismatch | Migrate only the owned destination with `storage init` |

**Credentials.**
- **Renewing:** renew source tokens for the same user, then restart the worker
  and API. Exporting a variable in another shell doesn't update a running process.
- **Rotating keys:** this invalidates tokens and cursors.

## PostgreSQL, disk and shutdown

**PostgreSQL.**
- **Version:** fixtures use digest-pinned PostgreSQL 17.12 in the `*-pg17` volumes.
- **Old volumes:** the old 14 volumes are untouched. Moving to 17 means a logical
  backup and restore. Never mount a 14 data directory on 17.

**Disk.**
- **Commands:** `df -h /`, `docker system df` and `scripts/dev.py check resources`.
- **Log limits:** container logs are capped at 10 MiB × 3 files, at INFO level.
  Unrotated DEBUG logs once filled 176 GiB.
- **Don't:** truncate Docker logs or prune volumes.

**Shutdown.** Stop the CLI processes first, then:

```bash
python3 scripts/dev.py pqs stop        # and --visibility if running
python3 scripts/dev.py storage stop
python3 scripts/dev.py localnet stop
```

**Reporting.**
- **Failures:** report with versions, the command and a synthetic reproduction.
- **Security issues:** use
  [private reporting](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md).
