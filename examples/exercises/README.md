# Exercise evidence fixture (M0)

Generic Daml controls for [source feasibility](../../docs/design/exercise-source.md).
This example has no tokens, markets or Outcomia-specific processing.

| Control | Expected IDE-ledger result | What still needs source verification |
| --- | --- | --- |
| `Factory_Pair` | Two children, Alice-only/Bob-only stakeholder views | Nonconsuming exercise witnesses versus child witnesses |
| `Factory_Nested` | Old factory consumed; successor keeps `factoryId`; one permanent child | Nested consuming exercise, transient relay/child and node boundaries |
| `Factory_Reject` | Failed submission leaves no child | No committed source receipt for rejected internals |
| Direct `Child` creation | Bob can reuse a declared factory reference | Matching text must never prove ancestry |
| Alice exercises operator choice | Submission rejected | Observing a factory does not grant choice authority |

Every template, choice and test has comments explaining its role. The scripts
check final contract visibility; they do not establish Ledger Effects witnesses,
PQS redaction behavior or direct-parent evidence.

## Offline checks

Install the declared open-source SDK 3.7.1. Its manifest selects damlc and
daml-script components 3.7.0; these component versions are expected.

From the repository root:

```bash
python3 scripts/dev.py check exercises --fixture-tests
# If DPM is installed outside PATH:
python3 scripts/dev.py check exercises --fixture-tests --dpm "$HOME/.dpm/bin/dpm"
```

The helper builds contracts and runs the two scripts on the IDE ledger in a
temporary `/tmp` directory, without an external Ledger connection. It copies only
example sources, excludes `.daml` output and removes the temporary directory even
on failure. This avoids DPM 1.1.0's observed space-encoded DAR dependency path.
No SDK-cache repair, upload, party provisioning or real transaction occurs.

## Retained PQS catalog

With the existing fixture database and its restricted reader available:

```bash
python3 scripts/dev.py check exercises
```

This executes bounded catalog SELECTs in a read-only snapshot, never event
functions or contract rows. It saves owner-only evidence under ignored
`.local/exercise-source/` and prints schema facts. It does not start services,
grant rights or claim current source authorization.

## Opt-in LocalNet experiment

Run only after the maintainer authorizes fixture writes. The experiment uploads
the contracts DAR, allocates three synthetic parties, provisions a writer and
four exact read-only source users, and submits the controls above. It creates
isolated PQS databases and SQL readers; existing source profiles stay intact.

```bash
python3 scripts/dev.py localnet up
docker compose -p cantongrid-pqs -f dev/pqs.compose.yaml up -d --no-deps --wait pqs-postgres
(cd examples/exercises/contracts && dpm build)
python3 scripts/dev.py check exercises --localnet --allow-ledger-writes
```

The exporter runs in `TransactionTreeStream` mode. Operator, Alice, Bob and
Alice+Bob sources run sequentially, each in its own database with exact party
filters and `CanReadAs` grants. Every event SELECT uses that database's restricted
reader; setup uses owner rights only to provision these new fixture databases.
Health and source rights are checked; failures do not fall back to owner reads.

Results stay in `.local/exercise-source/runs/cg-m0-<id>/`. If source setup fails
after the controls commit, resume without allocating parties or resubmitting:

```bash
# Replace this example ID with the run_id in the saved summary/directory name.
python3 scripts/dev.py check exercises --localnet --allow-ledger-writes --resume cg-m0-01234567
```

The helper stops its exporter on success/failure. Named databases, parties and
users remain for inspection. Stop LocalNet and the fixture PostgreSQL separately
when finished; preserve their volumes. Logs rotate at 10 MB × 3 per service.

The recorded [evidence](evidence.json) compares all five committed transactions
through authorized Ledger Effects and PQS, including partial-party views, one-row
keyset pages and overlapping replay. Keep raw payloads, tokens, party IDs and
contract IDs private. Missing exercises or descendants mean incomplete evidence,
never an inferred parent. This fixture does not verify pruning or redaction.

## Rust worker check (M1)

With the existing approved M0 run, LocalNet and fixture PostgreSQL running:

```bash
python3 scripts/dev.py storage up
cargo build -p cantongrid-cli --locked
# Replace the example ID with your existing M0 run; port 18083 must be free.
python3 scripts/dev.py check exercises --worker --allow-ledger-writes --resume cg-m0-01234567
```

This reuses the existing contracts/parties/users without provisioning or changing
rights. It starts an operator-scoped tree-mode exporter, initializes the owned
destination explicitly, indexes a fresh snapshot, and submits the same approved
pair/nested choices. Each Rust invocation renews real Ledger authorization and
reopens its durable checkpoint. An idle restart must leave counts unchanged.

Expected: four exercise receipts and four internal outbox entries; three consuming
exercises but only one lifecycle archive. The transient relay/child have no
indexed prior create; their receipts do not fabricate contract state. See
[worker evidence](worker-evidence.json). The helper stops its exporter; LocalNet
and both PostgreSQL services are stopped separately, preserving volumes.

## Direct lineage checks (M2)

Use an existing approved M0 run and the same running services as M1. These
commands submit local fixture transactions, so obtain authorization first.
Replace the example run ID; each invocation uses a fresh destination generation.

```bash
python3 scripts/dev.py check exercises --worker --lineage --scope operator --allow-ledger-writes --resume cg-m0-01234567
python3 scripts/dev.py check exercises --worker --lineage --scope ab --allow-ledger-writes --resume cg-m0-01234567
# Requires the existing Outcomia package already deployed on this LocalNet:
python3 scripts/dev.py check exercises --worker --lineage --outcomia --scope operator --allow-ledger-writes --resume cg-m0-01234567
python3 scripts/dev.py check exercises --worker --lineage --outcomia --scope ab --allow-ledger-writes --resume cg-m0-01234567
```

The helpers reuse existing parties, users and grants. They never upload an
Outcomia DAR or change its source repository. The Outcomia controls create a
protocol and draft binary market; they do not fund accounts or trade. The runtime
remains generic; package selection and transactions live only in the example helper.

Each check runs the real Rust worker, then compares authenticated REST and
GraphQL edges under the selected caller scopes. It checks pagination, unavailable
roots and caller revocation. The [recorded results](lineage-evidence.json) include
both generic and Outcomia runs. Restricted protocol observers do not acquire
visibility into an unindexed market endpoint. Transient creations can support
internal proof but do not become materialized public contracts.

See the [lineage guide](../../docs/guide/lineage.md) for configuration, partial
coverage and endpoint examples. Raw identifiers and credentials stay under
ignored `.local/`; the public evidence contains counts only. Helpers stop their
exporter and API child; stop LocalNet and both PostgreSQL services separately,
preserving volumes.
