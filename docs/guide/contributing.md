# Contribute and publish this guide

## Work on the suite

Read the repository's CONTRIBUTING.md and AGENTS.md before changing runtime
behavior. The main module boundaries are:

| Directory | Responsibility |
| --- | --- |
| `crates/core` | Exact values, configuration, canonical lifecycle and visibility |
| `crates/ledger` | Current source user/grant checks |
| `crates/pqs` | Read-only source paging and tested SQL compatibility |
| `crates/postgres` | Shared database connection restrictions |
| `crates/storage` | Owned migrations, transactions, checkpoints and outbox |
| `crates/models` | Model validation, exact evaluation and SQL plans |
| `crates/runtime` | Live worker, independent caller auth and REST/GraphQL |
| `crates/cli` | Command dispatch and operator workflows |
| `scripts/devtools` | Local setup/research helpers; not product processing |

Use small commits. Document data structures and functions, explain invariants and
include calculation examples where they clarify precision or business meaning.
Keep application choices in definitions rather than adding Outcomia-specific Rust.
New runtime dependencies require maintainer review/approval.

Credential-free checks:

```bash
cargo +1.100.0 fmt --all -- --check
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/dev.py release check
python3 scripts/dev.py check resources
```

Read CONTRIBUTING.md for owned-Postgres tests and security tooling. Source-dependent
tests require explicit matching credentials; ignored tests are not evidence of a
pass until executed. Separate fixture, owned-Postgres and LocalNet evidence in PRs.

## Edit the book

The guide is plain Markdown under `docs/guide/`. Navigation lives in
`docs/SUMMARY.md`. The repository-root `.gitbook.yaml` selects `docs/` as the content
root and `guide/README.md` as the first page. Each navigation page appears once.
Technical reference pages remain beside the guide to preserve their existing paths.

Keep examples executable from the repository root, use synthetic values and state
prerequisites. Match CLI syntax and limits to the implementation. Model examples
must state that preview/planning does not install or maintain a live projection.
Do not add tokens, local party IDs, ignored configuration or private payloads.

## Connect to GitBook

This checkout is ready for Git Sync; it does not create or publish a hosted space.
An owner can connect it after the documentation branch is pushed:

1. In your GitBook space, configure GitHub Git Sync for the CantonGrid repository
   and the branch containing these docs. For this layout, map the space to the
   repository root so the root `.gitbook.yaml` is used.
2. Import the repository content into the space. Confirm that the developer guide
   is the first page and that the navigation groups match `docs/SUMMARY.md`.
3. Preview internal links, code fences and tables. Only publish after reviewing the
   imported content and the space's access settings.
4. When the branch merges, point sync at the maintained release/default branch
   and confirm the imported version matches the suite developers are using.

Site-level `gitbook-docs.yaml` mapping, if your GitBook site already uses it,
selects the space directory before `.gitbook.yaml` applies. Keep that mapping at
the repository root for this configuration, or update the content paths together.
See GitBook's [official Git Sync configuration](https://gitbook.com/docs/docs-as-code/git-sync/content-configuration.md).
No GitBook token, hosted subscription or new runtime dependency is required to
read or contribute to this Markdown guide locally.
