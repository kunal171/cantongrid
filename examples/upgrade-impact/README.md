# Offline upgrade impact example

Uses the existing PoC contract and DAR. No LocalNet startup, uploads, party setup
or ledger writes. Build the PoC DAR first with SDK 3.7.1 (`dpm build` inside
`examples/poc`), then run from the repository root:

```bash
cargo build -p cantongrid-cli --locked
mkdir -p .local/impact
export CANTONGRID_IMPACT_DAR="$PWD/examples/poc/.daml/dist/cantongrid-poc-0.1.0.dar"
target/debug/cantongrid metadata inspect "$CANTONGRID_IMPACT_DAR" > .local/impact/metadata.json
python3 - <<'PY'
import json
from pathlib import Path

# Bind the canonical PoC selection to this verified build, preserving its expressions.
metadata = json.loads(Path('.local/impact/metadata.json').read_text())
models = json.loads(Path('examples/poc/models.json').read_text())
for model in models['models']:
    model['template']['package_id'] = metadata['main_package_id']
Path('.local/impact/models.json').write_text(json.dumps(models, indent=2) + '\n')
PY

target/debug/cantongrid sync plan "$CANTONGRID_IMPACT_DAR" "$CANTONGRID_IMPACT_DAR" \
  .local/impact/models.json > .local/impact/compatibility.json
target/debug/cantongrid sync summary "$CANTONGRID_IMPACT_DAR" "$CANTONGRID_IMPACT_DAR" \
  .local/impact/models.json
target/debug/cantongrid sync evaluate "$CANTONGRID_IMPACT_DAR" "$CANTONGRID_IMPACT_DAR" \
  .local/impact/models.json examples/upgrade-impact/samples.json
```

The same verified DAR is used twice to establish the unchanged baseline:
`compatible: true`, `rebuild: unnecessary`. The sample changes an Int64 near its
maximum by exactly one, so `projected_values_equal` is false. No values are echoed.
For an actual upgrade, replace the second DAR with the proposed artifact.

Executable checks:

```bash
cargo test -p cantongrid-sync --locked
cargo test -p cantongrid-cli --test sync_dar --locked -- --ignored
```

The first includes incompatible scale/optionality/removal/enum, incoming policy
pin drift, missing join target, redaction and resource-limit controls using
synthetic LF signatures. The second verifies the public CLI with the built PoC
DAR, JSON/summary/bundle parity, exact sample comparison and overwrite refusal.
It is an offline unchanged-DAR baseline, not evidence of a deployed upgrade.

See the [guide](../../docs/guide/upgrades.md#analyse-impact) for report fields and activation gates.
