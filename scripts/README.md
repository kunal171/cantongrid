# Development tools

Use one entry point from the repository root: `python3 scripts/dev.py --help`.
These are LocalNet research/provisioning/check helpers, not the Rust indexer.
The credential-free product workflow is documented in [CLI](../docs/cli.md).

| Command | Purpose | Side effects |
| --- | --- | --- |
| `dev.py localnet prepare/up/status/stop/logs/images` | Pinned upstream LocalNet lifecycle | Prepare/start/stop owned containers; retain named volumes. |
| `dev.py pqs up/status/stop/reader` | One-party provider source lifecycle and reader ACLs | Owned source user/config/containers; reader installs local SQL ACLs. |
| `dev.py pqs up/status/stop --visibility` | Separate AB visibility source | Owned AB source/config/database/containers; never the provider store. |
| `dev.py visibility setup` | Seed the owned Daml visibility scenario | Explicitly uploads its DAR, provisions parties/users and creates fixture records. |
| `dev.py validate-dar <file>` | Validate an existing DAR | Validation only; no upload/vetting. |
| `dev.py check ledger` | Health/authentication checks | Read-only HTTP calls. |
| `dev.py check pqs [--at-offset N]` | Scoped SQL/source compatibility | Read-only HTTP/SQL; requires live healthy source. |
| `dev.py check reader` | Verify real SQL privileges | Rollback-only denied write probes; no data changes. |
| `dev.py check visibility` | A/B/AB reads and rights-revocation controls | Temporarily revokes/restores only owned fixture grants. |
| `dev.py check visibility-pqs` | AB cache and live-source revocation | Temporarily revokes/restores owned grants and recreates the AB source. |

`python3 -m unittest discover -s scripts/tests -v` checks helper boundaries without
Docker or HTTP. Each route supports `--help` (for example, `dev.py check pqs --help`).

## Script review

The previous ten executable entry points are consolidated into this dispatcher;
the two PQS lifecycle scripts are merged into `devtools/pqs.py`. Shared Compose,
SQL command construction, repository root/environment and restrictive file writes
live in `devtools/common.py`. Loopback-only HTTP/JWT helpers live in `ledger.py`;
visibility scenario definitions live in `visibility.py`. Domain checks stay in
`devtools/checks/` rather than being mixed with source provisioning.

No compatibility experiment is discarded: source health, SQL reader denial,
exact values, Ledger visibility and retained-cache revocation test different
boundaries. Both source profiles remain separate even though their implementation
is shared. Existing docs now use the new commands; old script names are removed
instead of retaining redundant compatibility wrappers.

The [PQS schema-collision reproduction](../repros/pqs-schema-type-collision/README.md)
stays independent. It must run without importing CantonGrid or starting LocalNet,
so merging it with these helpers would weaken its upstream reproduction value.
Python cache/build/output files remain ignored.

## Owned Rust destination

`dev.py storage up/status/stop` manages the separate PostgreSQL destination on
loopback 5442. It uses a restricted owner role, dedicated volume and explicit
operator bootstrap SQL in `dev/storage-init.sql`. It does not share the source's
role, database or network. Stop retains its volume. Rust `storage init` owns the
CantonGrid schema; the Python helper does not implement ingestion or migrations.
See [durable fixture commands](../docs/cli.md).

## Security tools

`dev.py security install` verifies pinned upstream archive/executable hashes into
ignored `.local/security-tools` on Linux x86_64. `dev.py security check` runs all
checks; append `audit`, `dependencies`, `secrets` or `workflows` for one area.
Secret scans cover complete Git history plus eligible working-tree files and
redact output. Ignored credentials/build caches are not exported to the scanner.
See [contributing](../CONTRIBUTING.md) for versions and other-host instructions.
