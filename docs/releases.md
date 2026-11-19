# Releases and publication

Latest GitHub source prerelease: **0.3.0-alpha.4**. The current checkout is
**0.3.0-alpha.5, unreleased**. [CHANGELOG.md](https://github.com/kunal171/cantongrid/blob/main/CHANGELOG.md)
is the maintained change history; the links below retain each release's original notes.

| Version | Delivered scope | Destination migrations |
| --- | --- | --- |
| 0.3.0-alpha.5 (unreleased) | Staged model rebuilds and security fixes | 001–006 |
| [0.3.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.4) | Exact filters, native sorting, discovery and Outcomia query acceptance | 001–005 |
| [0.3.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.2) | Runnable Daml PoC, generated GraphQL and TypeScript interfaces | 001–005 |
| [0.3.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.1) | Model compiler, registry, installation and atomic live projections | 001–005 |
| [0.2.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.2.0-alpha.1) | LocalNet ingestion and authenticated generic APIs | 001–002 |
| [0.1.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.1.0-alpha.1) | Fixtures, owned storage and source checks | 001 |

Alpha.3 introduced crate packaging between source releases. The recorded alpha.4
registry check found only `cantongrid-core 0.3.0-alpha.3` published. Check the registry
before uploading or resuming a publication; a GitHub release does not upload crates.
Build the CLI from source using the [quickstart](guide/quickstart.md).

Current source requires explicit `storage init` through migration 006. Earlier
migration SQL is unchanged; older binaries reject unknown migration sets.
Do not downgrade a destination without a reviewed migration plan.

## Cut a source release

1. Update the changelog with delivered scope, compatibility, evidence and limits.
2. Run the [contributor checks](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md),
   including security and package verification. Source-dependent checks need matching infrastructure.
3. Review dependency notices, configuration/schema changes and migration checksums.
4. Merge a focused PR after CI passes, preserving the small commits.
5. Tag the verified commit and create its GitHub prerelease. Never overwrite tags.

For the current unreleased version, metadata validation is:

```bash
python3 scripts/dev.py release check --tag v0.3.0-alpha.5
```

This does not create a tag, release or deployment. Binary distribution needs a
separate packaging and dependency-notice review.

## Publish crates

The workspace permits crates.io publication only. Each package includes its
README, license, notice and compile-time assets. The release guard compares asset
copies with canonical examples; archive verification checks clean provenance,
legal files and self-contained inputs. See [crate responsibilities](architecture.md#modules).

From a clean commit:

```bash
python3 scripts/dev.py release check
cargo publish --workspace --dry-run --locked
python3 scripts/dev.py packages verify
```

CI verifies these archives without uploading them. Publication requires an
explicit operator action and local registry credentials. Configure them with
`cargo login --registry crates-io`; never paste credentials into chat or commit them.
After the checks, the upload command is:

```bash
cargo publish --workspace --locked
```

Cargo orders dependencies before their consumers. If publication stops partway,
check each registry version and resume only unpublished packages; versions are
immutable. Verify CLI installation outside the checkout after a coordinated
release. A core-only upload cannot install the CLI.
