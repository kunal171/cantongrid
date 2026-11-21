# Releases and publication

Latest stable release: **0.3.0**. Latest prerelease: **0.4.0-alpha.2**. [CHANGELOG.md](https://github.com/kunal171/cantongrid/blob/main/CHANGELOG.md)
is the maintained change history; the links below retain each release's original notes.

| Version | Delivered scope | Destination migrations |
| --- | --- | --- |
| [0.4.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.2) | DAR metadata, compound sorting, package upgrades, remote HTTPS and schema sync | 001–008 |
| [0.4.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.1) | Grouped counts/exact sums, generated APIs and full-source scope checks | 001–006 |
| [0.3.0](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0) | PostgreSQL 17.12 compatibility and large LocalNet staged rebuild acceptance | 001–006 |
| [0.3.0-alpha.5](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.5) | Staged model rebuilds and security fixes | 001–006 |
| [0.3.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.4) | Exact filters, native sorting, discovery and Outcomia query acceptance | 001–005 |
| [0.3.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.2) | Runnable Daml PoC, generated GraphQL and TypeScript interfaces | 001–005 |
| [0.3.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.1) | Model compiler, registry, installation and atomic live projections | 001–005 |
| [0.2.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.2.0-alpha.1) | LocalNet ingestion and authenticated generic APIs | 001–002 |
| [0.1.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.1.0-alpha.1) | Fixtures, owned storage and source checks | 001 |

The [CLI](https://crates.io/crates/cantongrid-cli/0.4.0-alpha.1) and all seven
libraries are published at **0.4.0-alpha.1**, with package-specific READMEs.
Stable **0.3.0** remains available.
Use the [quickstart](guide/quickstart.md) to install the CLI or build from source.

Alpha.3 introduced packaging between source releases. Historical crate packages
use `release/crates-<version>` branches with crate-local assets and publication
metadata; the original source tags are unchanged. The alpha.3 packages use the
same source baseline as its previously published core crate. Check the registry
before resuming an upload; GitHub releases do not upload crates automatically.

Current source requires explicit `storage init` through migration 006. Earlier
migration SQL is unchanged; older binaries reject unknown migration sets.
Do not downgrade a destination without a reviewed migration plan.

## 0.4.0-alpha.1 compatibility

This prerelease adds optional aggregate declarations. Upgrade all workers
and API instances before activating one; 0.3.0 rejects these declarations.
Existing declarations retain their fingerprints. Migrations 001–006 are unchanged.
Aggregate reads require the complete configured source-party scope; totals are
computed over maintained contributions at read time. See the [guide](guide/aggregates.md)
and [verification](verification.md) for precision, tested scope and limits.
Install the prerelease explicitly:

```bash
cargo install cantongrid-cli --version 0.4.0-alpha.1 --locked
```

## 0.3.0 compatibility

The runtime API and migrations remain the alpha.5 contract; migrations 001–006
are unchanged. Local fixture images move to pinned PostgreSQL 17.12 and new volume
names. Existing PostgreSQL 14 volumes need an explicit logical restore into those
new volumes; `up` does not migrate data. Read the [upgrade procedure](guide/operations.md#postgresql-support)
before restarting an existing setup.

This regular release covers the verified loopback LocalNet contracts profile.
Funded trading, joins/aggregates, streams, remote/TLS and production acceptance
are outside its scope.

## Cut a source release

1. Update the changelog with delivered scope, compatibility, evidence and limits.
2. Run the [contributor checks](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md),
   including security and package verification. Source-dependent checks need matching infrastructure.
3. Review dependency notices, configuration/schema changes and migration checksums.
4. Merge a focused PR after CI passes, preserving the small commits.
5. Tag the verified commit and publish its GitHub release. Mark alpha/beta builds
   as prereleases; regular releases use the reviewed release scope. Never overwrite tags.

For the current checkout, metadata validation is:

```bash
python3 scripts/dev.py release check
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
immutable. For HTTP 429, wait until the registry's stated retry time.
Verify CLI installation outside the checkout after a coordinated
release. A core-only upload cannot install the CLI.
