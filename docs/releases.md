# Releases and publication

Latest stable release: **0.4.0**. Latest prerelease: **0.5.0-alpha.1**. [CHANGELOG.md](https://github.com/kunal171/cantongrid/blob/main/CHANGELOG.md)
is the maintained change history; the links below retain each release's original notes.

Release **0.4.0** promotes the tested alpha.7 implementation for the node-local
contracts profile. Runtime code and migrations are unchanged from alpha.7.

| Version | Delivered scope | Destination migrations |
| --- | --- | --- |
| [0.5.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.5.0-alpha.1) | Offline policies, model preflight and bounded storage-only indexed reconciliation | 001–012 |
| [0.4.0](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0) | Verified node-local profile, scoped APIs/composition and durable canonical bootstrap | 001–012 |
| [0.4.0-alpha.7](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.7) | Atomic canonical bootstrap and opaque pinned snapshot paging | 001–012 |
| [0.4.0-alpha.6](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.6) | Canonical as-of reads, durable lifecycle feed and local hardening | 001–011 |
| [0.4.0-alpha.5](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.5) | Scoped developer inspection, upgrade impact/dependency reports and local sample evaluation | 001–010 |
| [0.4.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.4) | Scoped direct lineage, declared relationships and typed inner joins | 001–010 |
| [0.4.0-alpha.3](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.3) | Atomic private exercise receipts, scoped source evidence and LocalNet worker controls | 001–009 |
| [0.4.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.2) | DAR metadata, compound sorting, package upgrades, remote HTTPS and schema sync | 001–008 |
| [0.4.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0-alpha.1) | Grouped counts/exact sums, generated APIs and full-source scope checks | 001–006 |
| [0.3.0](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0) | PostgreSQL 17.12 compatibility and large LocalNet staged rebuild acceptance | 001–006 |
| [0.3.0-alpha.5](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.5) | Staged model rebuilds and security fixes | 001–006 |
| [0.3.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.4) | Exact filters, native sorting, discovery and Outcomia query acceptance | 001–005 |
| [0.3.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.2) | Runnable Daml PoC, generated GraphQL and TypeScript interfaces | 001–005 |
| [0.3.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.1) | Model compiler, registry, installation and atomic live projections | 001–005 |
| [0.2.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.2.0-alpha.1) | LocalNet ingestion and authenticated generic APIs | 001–002 |
| [0.1.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.1.0-alpha.1) | Fixtures, owned storage and source checks | 001 |

The CLI and nine libraries form the coordinated **0.4.0** release, with
package-specific READMEs. Use the [quickstart](guide/quickstart.md) or install the
release explicitly:

```bash
cargo install cantongrid-cli --version 0.4.0 --locked
```

Historical `release/crates-<version>` branches contain publication packaging for
older source releases. GitHub releases do not upload crates automatically.

## Unreleased 0.5.0-alpha.2

Adds optional `reconciliation_file` configuration and authenticated REST/GraphQL
over server-pinned policies. Existing configuration remains valid when omitted.
Upgrade binaries before adding the setting; older versions reject it. No migration,
reindex or dependency change. Bounds and report semantics remain those of alpha.1.
Acceptance includes owned PostgreSQL/mock controls and seven retained LocalNet
contracts with real read-only authorization. Remote reconciliation remains unverified. See the [API guide](guide/reconciliation.md#authenticated-api-050-alpha2-source).

## 0.5.0-alpha.1 reconciliation scope

The source branch adds offline exact policies/fixtures, model-schema preflight and
storage-only indexed per-contract reconciliation. Indexed reads require complete
source/caller scope, a completed retained prefix, exact active model revision and
consistent projections; empty, stale or incomplete evidence cannot yield all-clear.
No new migration or dependency. Public reconciliation APIs, persisted results,
cross-contract totals and asset/unit inference are not implemented. Adding
`StoreError::ReconciliationUnavailable` requires updating exhaustive downstream
error matches. This is a prerelease; stable 0.4.0 remains available.
See the [guide](guide/reconciliation.md).

## 0.4.0 supported scope and acceptance

This is a regular 0.x release of the tested node-local contracts profile, promoted
from alpha.7 without changing runtime code or migration SQL. It does not mark
all Phase 4 or M9 remote/production gates complete.

| Area | Supported scope / evidence |
| --- | --- |
| Live source profile | Canton 3.5.7, Scribe 3.5.4/schema041, PostgreSQL 17.12, authorized node-local PQS and snapshot-forward partial history |
| Core ingestion/API | Current source/caller grants, exact contract values, durable checkpoints, authenticated REST/GraphQL; retained LocalNet acceptance |
| Composition/debugging | Relationships, joins, aggregates and inspector controls have per-feature owned-PostgreSQL/mock-source evidence; see verification |
| Delivery/bootstrap | At-least-once canonical lifecycle feed, private snapshot pairing, replay/ACK/retention guards; owned SQL/HTTP interleaving and retained LocalNet baseline/restart controls |
| Bounded experimental features | Canonical temporal reconstruction and bootstrap retain 4,096-page / 8,192-event / 64-baseline limits and explicit unavailable/gap outcomes |
| Unsupported or unverified | Historical model bindings, model CDC, interfaces/reassignments, hosted multi-tenancy, public Scan, live remote/TestNet and production deployment |

Release checks: PR CI, default Rust and owned PostgreSQL suites, read-only retained
LocalNet evidence, security/license checks, clean self-contained crate archives,
registry checksums/rendered READMEs and CLI smoke outside the checkout. The stable
promotion changes only coordinated versions and documentation; runtime and
migration files must compare equal to the alpha.7 tag. An optimized CLI build and
offline smoke are recorded separately from registry installation.

From 0.3.0, back up before explicit owned migrations through 012. From alpha.7,
no migration or reindex is needed. Upgrade matching binaries together. Never point
storage initialization at the PQS source. Rollback uses the matching pre-upgrade
database/configuration/keys. Local benchmark/restore checks do not establish
production capacity, RPO or RTO. Remote validation remains pending source access.

## 0.4.0-alpha.7 compatibility

This release adds [atomic canonical bootstrap](guide/bootstrap.md) and
migration **012**, leaving 001–011 unchanged. Back up first, stop older processes,
explicitly run `storage init` on the owned destination, and upgrade worker/API
binaries together. Older binaries reject migration 012. Rollback needs the
pre-upgrade database/configuration/keys; never delete migration records.

Bootstrap metadata is private. The feature does not widen temporal limits, add
model CDC or establish remote/production acceptance. Source-interval parsing is
shared and strict, including duplicate/unknown field rejection.

## 0.4.0-alpha.6 compatibility

This release adds M7 bounded canonical temporal reads, M8 durable REST
lifecycle subscriptions and M9 operator/local hardening checks. Current grants apply to all historical and stream reads.
Historical models, model CDC and an atomic snapshot/feed handoff are not included.
Remote/TestNet and production acceptance remain pending.

Migration **011** adds private delivery subscriptions/receipts and supporting
indexes. Migrations 001–010 remain unchanged. Back up the owned destination, stop
older processes, explicitly run `storage init`, then start matching binaries.
Source PQS is never migrated. Older binaries reject migration 011. Rollback means
restoring the pre-upgrade backup with its matching configuration/keys, not deleting
migration records. See [recovery](guide/recovery.md).

## 0.4.0-alpha.5 compatibility

M5 adds read-only transaction and projection inspection plus sanitized local
operator commands. No migration, plan change, model revision or reindex is
required. Migrations 001–010 remain unchanged. The new server adds diagnostic
metadata/GraphQL roots; use the matching generated client definitions. Existing
alpha.4 database state remains compatible. See the [guide](guide/inspector.md).

M6 extends offline sync with selected-field/dependency impact, generated API/table
identities, rebuild requirements, a concise summary and redacted before/after
sample evaluation. Target revision drift and missing join targets now return
structured blockers instead of losing the report or failing later in generation.
The compatibility report gains an additive versioned `impact` section; compatible
bundles add a hashed `impact.txt`. See [upgrade impact](guide/upgrade-impact.md).

## 0.4.0-alpha.4 compatibility

This prerelease adds opt-in privacy-safe direct lineage and private migration 010,
plus versioned declared relationships and opt-in typed inner joins over current
model projections.
Use a new generation with both exercise and lineage source settings; receipt-only
and legacy generations cannot silently adopt lineage. Run explicit `storage init`
and upgrade workers and API instances together. Older binaries reject migration
010; rollback requires a pre-upgrade backup and the previous binary.

The [join guide](guide/joins.md) covers projection budgets and visible-result cursors.
The [relationship guide](guide/relationships.md) describes model revision gates;
older binaries reject declarations. The [lineage guide](guide/lineage.md) describes partial one-hop coverage, visibility and
cursor rules. Generic and Outcomia LocalNet evidence is linked from
[verification](verification.md). M2 has recorded LocalNet acceptance; M3/M4 use owned PostgreSQL and mock-source
authority. No production acceptance is claimed.

## 0.4.0-alpha.3 compatibility

Run explicit `storage init` through migration 009 before opening the new binary.
Migrations 001–008 remain unchanged. Older binaries reject the new migration set;
rollback requires a pre-upgrade destination backup and the previous binary.

Exercise ingestion is opt-in with `exercise_source: "pqs041_ledger_effects"` and
requires a separately prepared tree-mode source and new explicit generation.
Legacy configurations retain create/archive polling. Package handoff for receipt
streams and public exercise/lineage APIs remain unsupported. See the
[receipt contract](design/exercise-ingestion.md) for bounds, evidence and rollout.

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
