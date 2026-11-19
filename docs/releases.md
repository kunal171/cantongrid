# Releases

> Packaging backport: this branch enables crates.io publication after archive
> verification, with per-crate READMEs, legal files and local assets. The
> original source tag and its publication status are unchanged. The release
> guard now checks the reviewed crates.io metadata policy.

| Version | Scope |
| --- | --- |
| [0.1.0-alpha.1](releases/0.1.0-alpha.1.md) | Source prerelease: fixture/storage foundations and source checks |
| [0.2.0-alpha.1](releases/0.2.0-alpha.1.md) | Source prerelease: LocalNet ingestion and authenticated generic APIs |
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
python3 scripts/dev.py release check --tag v0.3.0-alpha.1
```

These commands verify metadata only; they do not tag or publish. Binary packaging
and third-party notices need separate review. Cargo packages remain `publish = false`.
