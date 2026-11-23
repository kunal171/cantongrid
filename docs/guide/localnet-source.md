# LocalNet ingestion

Requires Docker/Compose, Python 3.11+ and the [built CLI](quickstart.md).
This profile uses Canton 3.5.7 and Scribe 3.5.4/schema041. It supports exact
contract templates and raw payloads; interfaces/reassignments are unsupported.
For authorized remote HTTPS setup, see [remote sources](remote-source.md).

## Start infrastructure

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

`prepare` pins Quickstart revision `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9`.
Wait for readiness and retry checks after startup. `pqs up` creates/verifies a
one-party CanReadAs user and a one-hour local credential under `.local/pqs/`.
`pqs reader` creates a separate SELECT-only SQL role. Neither uploads your DAR
nor creates application contracts; those need separate authorized setup.

| Service | Local endpoint |
| --- | --- |
| JSON Ledger API: App User / Provider / SV | `127.0.0.1:2975` / `3975` / `4975` |
| gRPC Ledger API: App User / Provider / SV | `127.0.0.1:2901` / `3901` / `4901` |
| Wallet UI: App User / Provider / SV | `wallet.localhost:12000` / `13000` / `14000` |
| LocalNet database / PQS source / owned destination | `127.0.0.1:5440` / `5441` / `5442` |
| PQS readiness / optional Visibility source readiness | `http://127.0.0.1:18080/readyz` / `18081/readyz` |

Do not run upstream Quickstart startup alongside this isolated stack. Source
and destination roles/databases are separate; never migrate the source.

## Configure your source

```bash
umask 077
mkdir -p .local/my-app
cp examples/runtime/project.json .local/my-app/runtime.json
cp examples/runtime/grants.json .local/my-app/grants.json
chmod 600 .local/my-app/*.json
```

Replace placeholders with verified metadata:

| Setting | Value |
| --- | --- |
| `authorization.endpoint/user/parties` | Existing Ledger endpoint, active user and its exact nonempty CanReadAs set |
| `authorization.source/network/generation` | Stable source identity and reviewed generation |
| `templates` | Exact package ID, module and entity |
| `source_health` | Matching PQS readiness URL |
| `api` | Loopback bind, organization issuer/audience, separate key references and relative grants file |

Keep `history: snapshot_forward_partial`, `archive_policy: prior_create_current_grants`,
`payload: raw_json` and `retention: retain_while_authorized`. Ingestion `page_size`
is 1–8. The example is the complete schema; do not add fixture or model properties.
`validate` accepts fixture configuration only; live commands validate runtime JSON.

## Credentials and worker

The following passwords are disposable local fixture defaults:

```bash
export CANTONGRID_PQS_URL='host=127.0.0.1 port=5441 user=cantongrid_pqs_reader password=local-pqs-reader dbname=pqs'
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
read -r -s -p 'Existing source bearer token: ' CANTONGRID_SOURCE_TOKEN
export CANTONGRID_SOURCE_TOKEN
printf '\n'
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid source inspect-pqs --database-env CANTONGRID_PQS_URL
cantongrid index .local/my-app/runtime.json --once
cantongrid index .local/my-app/runtime.json
```

The bearer must belong to the configured read-only user; an administrator token
is not a substitute. `--once` processes at most one page, not full catch-up.
Continuous indexing retries failed cycles without acknowledging their progress.
An initial snapshot reports `bootstrapping`; afterward coverage is `partial_history`.
Contracts created and archived before the snapshot are not reconstructed.

Configure [caller keys/grants](api.md), then start a separate terminal with the
same required environment:

```bash
cantongrid serve .local/my-app/runtime.json
```

For the standalone source-rights probe, save the runtime's `authorization` object
as `.local/my-app/source-grants.json`, then run
`cantongrid source verify-grants .local/my-app/source-grants.json`.
This checks source access, not API callers.

The read-only demonstration `CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check pipeline`
requires the existing authorized Visibility fixture and isolated reader. Its
setup and side effects are listed in [development tools](https://github.com/kunal171/cantongrid/blob/main/scripts/README.md).
For restart, token renewal and disk limits, see [operations](operations.md).
