# Read-only inspector example

Available in the alpha.5 source checkout. These commands use public synthetic
Visibility files and do not contact Canton or mutate a database:

```bash
cargo build -p cantongrid-cli --locked
./target/debug/cantongrid inspect evaluate examples/models/visibility.json records examples/models/visibility-payload.json
./target/debug/cantongrid inspect evaluate examples/models/visibility.json records examples/inspector/invalid-input.json
```

The first report says `matched`; the second says `invalid` with `errorCode:
"missing"`. Neither exports projected values, input paths or the label above.
An invalid finding is a successful diagnostic command (exit 0); malformed command
arguments, unavailable files/models or source authorization produce a nonzero exit.

To export a finding, redirect stdout to a private local file. The inspector does
not attach payloads or logs automatically. [Guide](../../docs/guide/operations.md#inspector).

Owned PostgreSQL/API controls:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo test -p cantongrid-storage inspector -- --ignored --test-threads=1
cargo test -p cantongrid-cli --test runtime inspector -- --ignored --test-threads=1
python3 scripts/dev.py storage stop
```

The retained-source reconciliation test also requires the existing approved M0
SQL databases/readers and `CANTONGRID_M0_RUN_DIR`, plus per-scope
`CANTONGRID_M1_OPERATOR_URL`, `_A_URL`, `_B_URL`, `_AB_URL`. It reads retained source
rows and writes/deletes only an isolated owned destination generation:

```bash
cargo test -p cantongrid-cli --test inspector_source -- --ignored
```

This uses synthetic leases over retained LocalNet data; it is not a fresh live
Ledger authorization check. No DAR, party, grant or ledger transaction is changed.
