# Contributing

Read [AGENTS.md](AGENTS.md), [architecture](docs/architecture.md),
[security](docs/security.md) and the [roadmap](docs/roadmap.md). The current work is
Phase 4 model integration; use [existing examples](docs/guide/models.md) rather
than adding application-specific branches.

## Setup and checks

Requirements: Rust/Cargo 1.100.0 with rustfmt/Clippy, Python 3.11+. Docker/Compose
with `up --wait` support is needed only for database/source-dependent checks.
`rust-toolchain.toml` pins the compiler and components for rustup users.

```bash
cargo +1.100.0 fmt --all -- --check
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/dev.py release check
python3 scripts/dev.py check resources
```

After dependencies are fetched, add `--offline` to Cargo checks. Keep Cargo.lock
committed. Check free disk space and reuse one Cargo target before large builds.

For the separate owned PostgreSQL fixture:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo +1.100.0 test -p cantongrid-storage --locked -- --ignored
cargo +1.100.0 test -p cantongrid-models --locked -- --ignored
cargo +1.100.0 test -p cantongrid-cli --test workflow durable_cli --locked -- --ignored
cargo +1.100.0 test -p cantongrid-cli --test runtime --locked -- --ignored
cargo +1.100.0 test -p cantongrid-pqs destination_owner_cannot_be_used_as_a_source_reader --locked -- --ignored
python3 scripts/dev.py storage stop
```

These are public disposable database defaults. Stop preserves data. LocalNet
checks need their own matching credentials and setup; see [tools](scripts/README.md).
An ignored test is not a passing test until executed.

## Change requirements

- Use small commits. Add regression tests for behavior, privacy, precision or
  recovery changes. Comment data structures, functions and non-obvious calculations.
- Keep source/caller permissions independent. Unknown rights fail closed; counts,
  cursors and errors must not reveal inaccessible records.
- Preserve exact values, source-local positions and atomic state/checkpoint/outbox.
  Do not infer missing history or archives.
- Applied migrations are immutable. New runtime dependencies need a purpose,
  compatibility/license review and maintainer approval.
- Keep secrets, local party IDs and private payloads out of git/logs/issues. Use
  synthetic fixtures and [private security reporting](SECURITY.md).
- Do not submit ledger transactions, upload DARs, change application parties/grants
  or modify another repository without authorization. Source readers stay read-only.

## Security and pull requests

On Linux x86_64, run the pinned security tooling:

```bash
python3 scripts/dev.py security install
python3 scripts/dev.py security check all
```

It checks RustSec, dependency licenses/sources, full-history and eligible-tree
secrets, and Actions syntax. No blanket advisory/scanner ignores. Other platforms
can run the same pinned upstream versions listed in `scripts/devtools/security.py`.

Target `main`. Explain the problem, changed behavior, validation and limits. Label
fixture, owned-Postgres and LocalNet evidence accurately. Follow the
[release checklist](docs/releases.md) before tagging. Contributions use Apache-2.0;
include provenance and required notices for third-party code.

Docs live under `docs/`; update `docs/SUMMARY.md` with navigation changes. Keep
commands executable, distinguish available/planned behavior, and fix links when
merging pages. [Publishing configuration](docs/README.md#publishing) uses the same
Markdown for GitBook.
