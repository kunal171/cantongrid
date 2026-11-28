# Releases

Latest stable: **0.4.0**. Latest prerelease: **0.5.0-alpha.2**.
[CHANGELOG.md](https://github.com/kunal171/cantongrid/blob/main/CHANGELOG.md) is
the full change history.

| Version | Delivered scope | Destination migrations |
| --- | --- | --- |
| [0.5.0-alpha.2](https://github.com/kunal171/cantongrid/releases/tag/v0.5.0-alpha.2) | Authenticated configured reconciliation REST/GraphQL and retained LocalNet verification | 001–012 |
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

```bash
cargo install cantongrid-cli --version 0.4.0 --locked          # stable
cargo install cantongrid-cli --version 0.5.0-alpha.2 --locked  # reconciliation prerelease
```

## 0.4.0 scope

0.4.0 promotes alpha.7 without changing runtime code or migrations. It doesn't
complete the remote or production gates.

| Area | Supported scope |
| --- | --- |
| Live source | Canton 3.5.7, Scribe 3.5.4 / schema041, PostgreSQL 17.12, node-local PQS, history from the snapshot forward (partial) |
| Core | Current source and caller grants, exact values, durable checkpoints, REST/GraphQL (retained LocalNet acceptance) |
| Composition and debugging | Relationships, joins, aggregates, inspector |
| Delivery | At-least-once canonical feed, atomic bootstrap, replay, ACK and retention guards |
| Experimental | Temporal reads and bootstrap, limited to 4,096 pages, 8,192 events and 64 baselines |
| Not supported or verified | Historical model bindings, model CDC, interfaces and reassignments, multi-tenancy, public Scan, remote and production |

Evidence for each feature is in [verification](verification.md).

## Upgrading

1. **Back up** the owned destination. Never point `storage init` at the PQS source.
2. **Stop** older workers and API processes.
3. **Migrate:** run `cantongrid storage init --database-env CANTONGRID_DESTINATION_URL`.
4. **Upgrade together:** upgrade every worker and API binary at the same time.

**Notes.**
- **No downgrades:** older binaries reject newer migrations. Rolling back means
  restoring the pre-upgrade database, config and keys. Never delete migration
  records.
- **From alpha.7 to 0.4.0:** no migration or reindex is needed.
- **0.5.0-alpha.1:** adds `StoreError::ReconciliationUnavailable`, so exhaustive
  matches downstream need updating.
- **0.5.0-alpha.2:** adds the optional `reconciliation_file` setting. Upgrade the
  binaries before setting it.

## Cutting a release

1. **Changelog:** update it with the scope, compatibility, evidence and limits.
2. **Checks:** run the
   [contributor checks](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md),
   including `python3 scripts/dev.py release check`.
3. **Review:** dependency notices, schema changes and migration checksums.
4. **Merge:** merge a focused PR after CI.
5. **Tag:** tag the verified commit and publish the GitHub release.
   - **Prereleases:** mark alphas as prereleases.
   - **Tags:** never overwrite a tag.

## Publishing crates

Only crates.io is used. Each package ships its README, license, notice and assets.

```bash
python3 scripts/dev.py release check
cargo publish --workspace --dry-run --locked
python3 scripts/dev.py packages verify
cargo publish --workspace --locked          # explicit operator action, after cargo login
```

- **Interrupted publish:** if it stops partway, publish only the missing versions.
  Versions are immutable.
- **HTTP 429:** wait for the retry time.
- **Afterwards:** check CLI installation outside the checkout.
