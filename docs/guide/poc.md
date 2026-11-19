# Run the LocalNet PoC

This example builds a small Daml prediction-market contract, uploads its DAR,
submits real choices and verifies their indexed state through generated GraphQL.
A local viewer shows current markets, lifecycle events, checkpoint and freshness.

Use [0.3.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.4)
for the query, revision and recovery checks described here.
The example and current documentation are maintained on
[`main`](https://github.com/kunal171/cantongrid/tree/main/examples/poc).
Use the release tag for a reproducible review:

```bash
git clone https://github.com/kunal171/cantongrid.git
cd cantongrid
git checkout v0.3.0-alpha.4
```

## Prerequisites

Use Linux with Docker/Compose, Python 3.11+, Git, the repository's Rust toolchain
and DPM with SDK 3.7.1 selected. Optional TypeScript validation needs `tsc`.
The tested source is Canton 3.5.7 and Scribe 3.5.4/schema041; the contract targets LF 2.1.

Run from the release checkout above. The following commands provision only
the owned disposable LocalNet. They create fixture parties/users, upload DARs and
submit test transactions. No funds or actual prediction trades are involved.

```bash
export PATH="$HOME/.dpm/bin:$PATH"
dpm install 3.7.1
# Build each package from its directory:
(cd examples/visibility && dpm build)
(cd examples/poc && dpm build)

# Reuse one Cargo target; a normal build produces target/debug/cantongrid.
cargo build -p cantongrid-cli --locked
export CANTONGRID_BIN="$PWD/target/debug/cantongrid"

python3 scripts/dev.py localnet prepare
python3 scripts/dev.py localnet up
python3 scripts/dev.py check ledger
python3 scripts/dev.py visibility setup
python3 scripts/dev.py pqs up
python3 scripts/dev.py pqs up --visibility
python3 scripts/dev.py pqs reader --visibility
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
"$CANTONGRID_BIN" storage init --database-env CANTONGRID_DESTINATION_URL
```

The shared Visibility setup creates two owned parties, a writer and separate
read-only source users. The PoC selects only `Market:PredictionMarket` in a new
source scope; existing Visibility contracts are not model inputs. Source SQL uses
a SELECT-only reader. The source and owned destination are separate databases.

Wait for readiness after startup. If an interrupted startup left Splice unhealthy
after PostgreSQL recovered, restart its owned container with
`docker restart cantongrid-localnet-splice`, then repeat `check ledger`.
See [LocalNet setup](localnet-source.md) and [disk/shutdown](operations.md#disk-and-shutdown).

## Run and verify

```bash
python3 examples/poc/demo.py run --allow-ledger-writes
```

The runner:

1. Reads the built DAR's actual package ID, validates/uploads it, and creates
   A-only, B-only and shared Draft markets.
2. Generates runtime configuration and client contracts from `models.json`.
3. Captures a source snapshot, registers/installs the model, and binds its backfill.
4. Creates four query-control contracts, starts the Rust indexer/API, publishes
   the shared market and waits for Open.
5. Stops the worker, resolves the market on Canton, restarts the worker and waits
   for Resolved. Consuming choices archive the previous contract revisions.
6. Verifies exact values, real event identities, caller privacy, cursor isolation,
   grant revocation and repeat observation without duplicate records.
7. Runs 54 filtered page sequences through REST/GraphQL, promotes and restores
   a model revision, then stops/restarts PQS to verify fail-closed recovery.

Expected output ends with `PASS` lines and the evidence path.
`.local/poc/report.json` contains the result; secrets and contract IDs remain in
owner-only local state files. A fresh run checks eleven create/archive identities, including the four query
controls. Saved query contracts are reused on later runs.
A repeat archives the previous resolved shared market and starts another cycle,
retaining history and database volumes. An interrupted Open cycle can be completed
with `transact --choice resolve` before rerunning acceptance. Review ambiguous
Ledger submission failures before retrying; no automatic source-state repair exists.

## Generated schema and types

The declaration is the source of truth. This is generation from a reviewed model
mapping, not arbitrary DAR-to-client-code generation.

```bash
"$CANTONGRID_BIN" models plan .local/poc/models.json
"$CANTONGRID_BIN" models typescript .local/poc/models.json
"$CANTONGRID_BIN" models graphql .local/poc/models.json
# Optional: validate the generated transport interfaces.
tsc --noEmit --strict --target es2020 --module esnext \
  --typeRoots .local/poc/generated/types-empty .local/poc/generated/models.ts
```

The runner saves `schema-plan.json`, `models.ts` and `schema.graphql` in
`.local/poc/generated/`. PostgreSQL uses native numeric/bigint columns. GraphQL
and TypeScript represent them as strings: `priceTick = "0.0000000001"`,
`priceTickBps = "0.0000010000"`, `quantityLot = "9223372036854775807"`.
The optional outcome is `boolean | null`.

Generated root/type names include the definition fingerprint. The model endpoint
is `POST /models/graphql`; `/graphql` continues to serve generic contracts/history.
Model queries require the namespace shown in `.local/poc/state.json` and a valid
organization bearer. The viewer/helper issues short-lived local fixture bearers
without printing signing keys. Inspect `support.query` and the generated SDL for
the complete query. Model API limits: 25 rows/page, two model resolvers/operation,
depth 8, complexity 100 and a 2 MiB response. Signed cursors bind caller, source,
grants, namespace and immutable model fingerprint.

## Query acceptance

`queries.py` submits four real contracts through the local writer. Prices 1, 2
and 10 expose string-based numeric sorting errors; quantities above 2^53 expose
floating-point conversion. Two equal prices check contract-ID tie breaking.
Null, false and true outcomes check nullable ordering and comparison semantics.

For A, B and AB separately, the runner compares actual contract identities,
filtered counts and exact values on every one-row page in both transports. It
checks discovery parity, cursor reuse between equivalent REST/GraphQL queries,
rejection after filter/sort/principal/grant changes, and invalid numeric inputs.
Revision 2 is registered/installed, activated and restored to revision 1 in a
`finally` block. An exporter outage must return 503 despite retained rows;
restart must restore access without moving the acknowledged source checkpoint.

The full run requires `--allow-ledger-writes`. `verify` reuses the contracts and
checks query/permission behavior without creating contracts, changing model
bindings or stopping PQS. It does update the local grants epoch during its cursor
control. `report.json` records the full-run results under `modelQueries`.

## Watch transactions

```bash
# Terminal 1: runs the Rust worker/API and local viewer until Ctrl+C.
python3 examples/poc/demo.py serve
```

Open **http://127.0.0.1:18891**. Alice sees her private market and the shared market;
Bob sees his private market and the shared market. After query acceptance, each
also sees three query-control rows: A=5, B=5, AB=7 active rows.
Switch identities to inspect the different state/history views. The viewer polls
every two seconds and clears data on identity changes or failed requests.
Its Python proxy is a local test-identity selector, not production sign-in.

```bash
# Terminal 2: observe each change in the viewer.
python3 examples/poc/demo.py transact --choice create --allow-ledger-writes
python3 examples/poc/demo.py transact --choice publish --allow-ledger-writes
python3 examples/poc/demo.py transact --choice resolve --allow-ledger-writes
python3 examples/poc/demo.py verify
```

`create` starts a new cycle after the last market resolved. Ctrl+C stops only
child processes started by the viewer; infrastructure and volumes remain.
The default API/viewer ports 18890/18891 must be free.

## Evidence and boundaries

The checked-in [sanitized evidence](https://github.com/kunal171/cantongrid/blob/main/examples/poc/evidence.json)
records an actual LocalNet run. Generated TypeScript compiled and Chromium
rendered the authorized viewer. Separate owned-PostgreSQL tests cover visibility
before counts/pages, stale revisions, unknown namespaces, explicit null cursors,
operation limits, caller revocation and source-health/permission failures.

The contract demonstrates lifecycle indexing. It has no collateral, matching or
settlement. History is snapshot-forward partial; the indexer cannot recover events
before its captured snapshot. API keys/users are local fixtures. Public streams,
joins/aggregates, staged large backfills, remote/TLS, devnet and production acceptance remain later
work. See [verification](../verification.md) for the earlier Outcomia evidence.
