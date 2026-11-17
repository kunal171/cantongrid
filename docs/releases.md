# Releases

Status: release-oriented development begins with `0.1.0-alpha.1`. No CantonGrid tag
or GitHub release existed when this plan was recorded. Alpha releases describe
the capabilities delivered at their exact commit; they do not mark all Phase 3 complete.

## Release milestones

| Version | Scope | Acceptance |
| --- | --- | --- |
| `0.1.0-alpha.1` | Reference lifecycle/configuration, owned PostgreSQL recovery, source-contract research, security CI and contributor setup. | Baseline/DB/security checks pass; precise fixture/LocalNet boundaries and open gates are recorded. |
| `0.2.0-alpha.1` | Authorized live contracts slice and generic authenticated REST/GraphQL. | LocalNet source/handoff, current grants, exact raw values, coverage, A/B/AB privacy and both actual examples verified; see [evidence](phase3-verification.md). |
| `0.3.0-alpha.1` (in progress) | Declarative models; compiler/evaluation/SQL planning delivered, live storage/generated APIs pending. | Phase 4 acceptance remains open; see [model contract](phase4-models.md). |
| Later | Authenticated durable streams. | Separate Phase 4/5 acceptance; do not fold them into the Phase 3 release. |

Keep feature changes in small reviewable commits on `release/<version>` or focused
feature branches into that release. Maintain [CHANGELOG.md](../CHANGELOG.md),
configuration/migration compatibility notes and [verification](verification.md).

## Release checklist

1. Freeze the intended scope and document outstanding live/data/permission gates.
2. Run formatting, Clippy, unit/executable tests, isolated PostgreSQL tests and
   security checks. Live tests require authorized infrastructure; absence is not a pass.
3. Review dependencies, licensing, schema/configuration compatibility and CLI changes.
   An alpha name does not waive missing permissions or third-party notice requirements.
4. Confirm the workspace version and release notes match the exact candidate commit.
5. Open a release PR; required CI must pass before merging, preserving focused commits.
6. Create an annotated version tag and GitHub release only for the verified commit.
   Never overwrite a published tag. Versioned release notes must distinguish fixture,
   owned-Postgres, LocalNet and unavailable production evidence.
7. Publish binaries only after license/third-party notices and reproducible packaging
   are ready. Cargo packages stay `publish = false` until packaging is reviewed.

A prerelease may ship a useful verified subset with explicit limits. A Phase 3
completion claim requires the full acceptance matrix, including actual Outcomia
integration or an explicit blocked status for unavailable contract data.

## Candidate metadata check

```bash
python3 scripts/dev.py release check
python3 scripts/dev.py release check --tag v0.2.0-alpha.1
cargo run -p cantongrid-cli -- --version
```

The check validates inherited workspace/lockfile versions, the project license and
notice, changelog entry and an optional intended tag. CI also checks pushed tag
names against the workspace version. It never creates a tag, publishes a release,
packages binaries or changes `publish = false`.
