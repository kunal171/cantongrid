# CantonGrid LocalNet PoC

A standalone Daml prediction-market lifecycle, its projection model, transaction
runner and local viewer. Product indexing and API execution use the Rust CLI.

Follow the [complete guide](../../docs/guide/poc.md) for startup, DAR deployment,
type generation, transactions, privacy and recovery checks.

- `daml/Market.daml`: Draft → Open → Resolved, with consuming choices.
- `models.json`: selected fields and exact `priceTick × 10000` calculation.
- `demo.py`: acceptance run, separate transactions, verification and viewer startup.
- `lifecycle.py` / `support.py`: local Ledger commands and shared fixture plumbing.
- `dashboard.py` / `dashboard.html`: loopback-only viewer using fixture identities.
- `evidence.json`: sanitized result of the recorded LocalNet run.

Generated files, secrets, contract identities and detailed reports stay ignored
under `.local/poc/`. No grant application is submitted by these tools.
