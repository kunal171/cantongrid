# LocalNet

**Profile:**
- **Software:** Canton 3.5.7 and Scribe 3.5.4 (schema041).
- **Supported:** exact contract templates and raw payloads.
- **Not supported:** interfaces and reassignments.

For a remote HTTPS source, see [operations](operations.md#remote-sources).

## Start the stack

Requirements: Docker Compose, Python 3.11+ and the [CLI](quickstart.md).

```bash
python3 scripts/dev.py localnet prepare     # pins Quickstart 8b898ed3
python3 scripts/dev.py localnet up
python3 scripts/dev.py check ledger
python3 scripts/dev.py pqs up               # one-party CanReadAs user, 1 h credential
python3 scripts/dev.py pqs reader           # separate SELECT-only SQL role
python3 scripts/dev.py check reader && python3 scripts/dev.py check pqs
python3 scripts/dev.py storage up
```

| Service | Endpoint |
| --- | --- |
| JSON Ledger API, App User / Provider / SV | `127.0.0.1:2975` / `3975` / `4975` |
| gRPC Ledger API | `2901` / `3901` / `4901` |
| LocalNet DB / PQS source / owned destination | `5440` / `5441` / `5442` |
| PQS readiness | `http://127.0.0.1:18080/readyz` |

**Rules:**
- **No upstream Quickstart:** don't run it alongside this stack.
- **Source:** it's never migrated.
- **Health:** if Splice stays unhealthy after a restart, run
  `docker restart cantongrid-localnet-splice`.

## Index your own contracts

```bash
umask 077 && mkdir -p .local/my-app
cp examples/runtime/project.json .local/my-app/runtime.json
cp examples/runtime/grants.json  .local/my-app/grants.json
```

Fill in `runtime.json`:

| Setting | Value |
| --- | --- |
| `authorization.endpoint/user/parties` | Ledger endpoint, an active user and its exact CanReadAs set |
| `authorization.source/network/generation` | A stable source identity |
| `templates` | Exact package ID, module and entity |
| `source_health` | The PQS readiness URL |
| `api` | Loopback bind, issuer and audience, key references, grants file |

**Fixed settings:**
- **Required values:** keep `history: snapshot_forward_partial`,
  `archive_policy: prior_create_current_grants`, `payload: raw_json` and
  `retention: retain_while_authorized`.
- **Page size:** `page_size` is 1–8.

```bash
export CANTONGRID_PQS_URL='host=127.0.0.1 port=5441 user=cantongrid_pqs_reader password=local-pqs-reader dbname=pqs'
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
read -r -s -p 'Source bearer: ' CANTONGRID_SOURCE_TOKEN; export CANTONGRID_SOURCE_TOKEN
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid source inspect-pqs --database-env CANTONGRID_PQS_URL
cantongrid index .local/my-app/runtime.json          # add --once for a single page
cantongrid serve .local/my-app/runtime.json          # separate terminal
```

- **Bearer:** it must belong to the configured read-only user. An admin token
  isn't a substitute.
- **History:** the first snapshot reports `bootstrapping`, and after that
  `partial_history`. Contracts that were created and archived before the snapshot
  aren't reconstructed.
- **Retries:** failed cycles retry without acknowledging progress.
- **Source rights:** `cantongrid source verify-grants <authorization.json>` checks
  source rights (not API callers).

## The PoC

`examples/poc` is a small Daml market contract.
- **What it shows:** real ledger writes, indexing, generated types and queries,
  private caller views, recovery and a browser viewer.
- **Version:** reproduce it at tag `v0.3.0`.
- **Tools:** it needs DPM with SDK 3.7.1.

```bash
dpm install 3.7.1
(cd examples/visibility && dpm build) && (cd examples/poc && dpm build)
cargo build -p cantongrid-cli --locked && export CANTONGRID_BIN="$PWD/target/debug/cantongrid"
python3 scripts/dev.py visibility setup
python3 scripts/dev.py pqs up --visibility && python3 scripts/dev.py pqs reader --visibility
python3 examples/poc/demo.py run --allow-ledger-writes
```

**What `run` does:**
1. **Deploy:** uploads the DAR and creates A-only, B-only and shared draft markets.
2. **Generate:** generates config and TypeScript/GraphQL from `models.json`,
   installs the model and starts the indexer and API.
3. **Lifecycle:** publishes and resolves the shared market, with the worker
   stopped in between.
4. **Values and privacy:** checks exact values, event identities, caller privacy
   and cursor isolation.
5. **Revocation:** checks grant revocation and that repeat observations add no
   duplicates.
6. **Queries:** runs 54 filtered page sequences over REST and GraphQL.
   - **Data:** the prices 1, 2 and 10, quantities above 2^53, and null, false and
     true outcomes.
   - **What they catch:** string sorting and float errors.
7. **Model revisions:** promotes and restores a model revision.
8. **Recovery:** stops PQS to check that reads fail closed, then recovers.

**Output:**
- **Result:** the run ends with `PASS` lines and writes `.local/poc/report.json`.
- **Artifacts:** generated files go to `.local/poc/generated/`.
- **Numbers:** numbers are strings in GraphQL/TS and native numeric in PostgreSQL.

**Other commands:**
- **Read-only check:** `demo.py verify` re-checks without writing to the ledger.
- **Viewer:** `demo.py serve` opens a viewer on `http://127.0.0.1:18891`.
  - **Who sees what:** Alice and Bob each see their own private market and the
    shared one.
  - **Driving it:** `demo.py transact --choice create|publish|resolve` drives the
    viewer.
- **Staged rebuild:** `demo.py rebuild --allow-ledger-writes` is the staged rebuild
  check.
  - **Size:** 261 contracts, above the 128-row atomic cap.
  - **Steps:** it resumes in 32-row steps, revokes and restores rights mid-rebuild,
    and then promotes and rolls back.

Evidence (`examples/poc/evidence.json`, `rebuild-evidence.json`) contains no party
IDs, contract IDs or credentials. The PoC has no collateral, matching or
settlement.
