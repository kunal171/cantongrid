# Releases

| Version | Scope |
| --- | --- |
| [0.1.0-alpha.1](releases/0.1.0-alpha.1.md) | Source prerelease: fixture/storage foundations and source checks |
| [0.2.0-alpha.1](releases/0.2.0-alpha.1.md) | Source prerelease: LocalNet ingestion and authenticated generic APIs |
| [0.3.0-alpha.2](releases/0.3.0-alpha.2.md) | Source prerelease: runnable PoC, generated model GraphQL and TypeScript interfaces |
| [0.3.0-alpha.1](releases/0.3.0-alpha.1.md) | Source prerelease: declarative models and bounded atomic live projections |

Published artifacts are listed on [GitHub Releases](https://github.com/kunal171/cantongrid/releases).
An alpha version describes its tested subset, not production readiness.
Current source requires explicit destination initialization through migration 005.
Older binaries that do not know this migration set reject it; 001–004 remain unchanged.

## Cut a release

1. Record delivered scope, compatibility, evidence and remaining limits in the
   changelog and release notes.
2. Run formatting, Clippy, fixture/owned-Postgres tests, security and release checks.
   Source-dependent tests need matching authorized infrastructure.
3. Review dependency licenses, config/schema changes and migration checksums.
4. Merge a focused PR after required CI passes, preserving the small commits.
5. Tag the verified commit and create its GitHub prerelease. Never overwrite tags.

```bash
python3 scripts/dev.py release check
python3 scripts/dev.py release check --tag v0.3.0-alpha.2
```

These commands verify metadata only; they do not tag or publish. Binary packaging
and third-party notices need separate review. The next alpha prepares crates.io
packages under the publication checks below.

## crates.io packages (0.3.0-alpha.3 preparation)

The workspace permits publishing only to crates.io. Every package includes its
README, Apache-2.0 license and notice. Crate-local `assets/` copies make fixture
and model tests self-contained; the release guard compares them with the
canonical files under `examples/` and rejects drift.

| Package | Purpose |
| --- | --- |
| `cantongrid-core` | Exact values, lifecycle, visibility and lease decisions |
| `cantongrid-postgres` | Restricted PostgreSQL connections and redacted errors |
| `cantongrid-ledger` | Current source authorization checks |
| `cantongrid-models` | Declarative models, evaluation and schema/client generation |
| `cantongrid-pqs` | Read-only source paging |
| `cantongrid-storage` | Owned migrations, canonical state and projections |
| `cantongrid-runtime` | Indexing workers and authenticated APIs |
| `cantongrid-cli` | The `cantongrid` executable |

Before upload, from a clean commit:

```bash
python3 scripts/dev.py release check
cargo publish --workspace --dry-run --locked
python3 scripts/dev.py packages verify
```

The dry run packages and builds all workspace members with a temporary registry
for unpublished internal dependencies. Archive checks reject dirty provenance,
missing compile-time inputs, unexpected files and changed license/notice content.
The CI workflow runs these checks without uploading anything.

Upload uses the operator's locally configured crates.io credentials. A first
publication needs an account with verified email and permission for these names.
Run `cargo login --registry crates-io` locally; never paste a token into chat or
commit it. After verification, the explicit operator command is:

```bash
cargo publish --workspace --locked
```

Cargo orders the workspace dependencies before their consumers. If publication
stops partway, check each registry version and resume only unpublished packages.
Versions are immutable. Do not overwrite release tags or bypass verification.
After the CLI is available, verify an installation outside the checkout:

```bash
cargo install cantongrid-cli --version 0.3.0-alpha.3 --locked
cantongrid --version
cantongrid init /tmp/cantongrid-installed-example
cantongrid fixture run /tmp/cantongrid-installed-example/cantongrid.json --party A
```

The alpha API and the LocalNet/runtime limits still apply. Publishing crates does
not establish remote deployment or production acceptance. Binary packaging has a
separate dependency-notice review.
