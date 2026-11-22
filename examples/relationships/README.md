# Explicit factory references

This offline model example uses field names from the generic
[ExerciseProbe Daml fixture](../exercises/contracts/daml/ExerciseProbe.daml).
The all-zero package IDs are placeholders, not deployed package metadata.

From the repository root, with the source CLI built:

```bash
cargo build -p cantongrid-cli --locked
./target/debug/cantongrid models validate examples/relationships/models.json
./target/debug/cantongrid models preview examples/relationships/models.json factories examples/relationships/factory.json
./target/debug/cantongrid models preview examples/relationships/models.json children examples/relationships/child.json
./target/debug/cantongrid models preview examples/relationships/models.json children examples/relationships/forged-child.json
./target/debug/cantongrid models graphql examples/relationships/models.json
./target/debug/cantongrid models typescript examples/relationships/models.json
```

All three payload previews expose the same `factoryKey`. The forged child's
reference therefore satisfies the *declared* equality policy if both endpoints
are indexed, active and independently visible. It cannot establish creation
ancestry. M2 source-proof lineage never reads this field or the declarations.

For live use, inspect the deployed DAR metadata, replace both package IDs,
select both templates and register/install/bind `children` and `factories` in one
namespace. Do not upload a DAR, change parties/grants or submit transactions just
to run this offline example. Query commands and revision handling are in the
[guide](../../docs/guide/relationships.md).

The owned PostgreSQL and mock-source API tests cover current-state lifecycle,
A/B/shared visibility, exact numeric keys, paging, replay, projection rollback,
policy/target publication and revocation. These tests are not a new LocalNet
acceptance claim. To reproduce them using the disposable owned database:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo test -p cantongrid-storage declared_links -- --ignored --test-threads=1
cargo test -p cantongrid-cli --test runtime declared_relationship -- --ignored
python3 scripts/dev.py storage stop
```

The disposable database password is a fixture value. Stop preserves its volumes.
