# Typed inner-join example

This extends the [relationship example](../relationships/README.md) with explicit
selected columns: `Factory.factoryKey` and `Child.label`. All-zero package IDs
are offline placeholders. The Daml field names come from
[ExerciseProbe](../exercises/contracts/daml/ExerciseProbe.daml).

```bash
cargo build -p cantongrid-cli --locked
./target/debug/cantongrid models validate examples/joins/models.json
./target/debug/cantongrid models plan examples/joins/models.json
./target/debug/cantongrid models preview examples/joins/models.json factories examples/relationships/factory.json
./target/debug/cantongrid models preview examples/joins/models.json children examples/relationships/child.json
./target/debug/cantongrid models graphql examples/joins/models.json
./target/debug/cantongrid models typescript examples/joins/models.json
```

For live use, resolve actual package metadata, select both templates and
register/install/bind both revisions in one namespace. No DAR upload, new party,
grant change or ledger transaction is needed for the offline commands above.
The [guide](../../docs/guide/composition.md#typed-inner-joins) covers runtime queries and publication.

A matching reference remains an application declaration, including a forged
reference. It never becomes M2 source ancestry. A source observer cannot read a
joined target unless independently authorized for that target.

To reproduce the PostgreSQL and mock-authority transport checks:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo test -p cantongrid-storage model_projection::joins -- --ignored --test-threads=1
cargo test -p cantongrid-cli --test runtime inner_join -- --ignored
python3 scripts/dev.py storage stop
```

The fixture password is disposable; stop preserves volumes. These checks do not
claim LocalNet or production acceptance.
