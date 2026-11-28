# Package metadata and upgrades

When a Daml package changes, upgrade in three steps.
1. **Inspect** the new DAR offline.
2. **Analyse** the impact on your models.
3. **Hand over** to the new package without losing history.

Inspection and analysis need no network, database or ledger credentials. Neither
one uploads a DAR, applies SQL or activates anything.

## Inspect a DAR

```bash
cantongrid metadata inspect examples/poc/.daml/dist/cantongrid-poc-0.1.0.dar > .local/poc-metadata.json
```

**What the JSON contains.**
- **Format:** `format_version: 1`.
- **Identity:** `artifact_sha256` (the whole DAR) and `main_package_id`.
- **Packages:** each with its name, version, LF version and upgrade predecessor.
- **Signatures:**
  - **Types:** records, variants, enums, interfaces and aliases, with the exact
    Numeric scales.
  - **Templates:** templates and their keys.
  - **Choices:** choices, with their consuming flag and argument and result types.
  - **Interfaces:** interface views and implementations.
  - **Dependencies:** `type_dependencies`.

**How identities work.**
- **Format:** a type identity is `{package_id, module, name}`.
- **Order:** field order is preserved.
- **Repacking:** it can change the artifact hash without changing any signature.

**Scope and limits.**
- **Accepted:** LF 2.1 patch 0 only.
- **Signatures, not execution:** expressions, controllers and signatories aren't
  extracted.
- **Size limits:** the DAR and the output are each capped at 32 MiB, the archive
  at 1,024 entries, and the content at 256 packages.
- **Expansion limits:** 8 MiB per expanded entry and 64 MiB in total.
- **Rejected:** unsafe or duplicate paths, CRC or hash mismatches, and dangling
  references.

## Analyse impact

```bash
cantongrid sync plan    old.dar new.dar models.json > compatibility.json
cantongrid sync summary old.dar new.dar models.json
cantongrid sync bundle  old.dar new.dar models.json reviewed-bundle   # refuses if blocked
cantongrid sync evaluate old.dar new.dar models.json samples.json
```

**The `impact` section reports, for each model:**
- **Fields:** each field's old and new type and semantics.
- **Dependencies:** filter, aggregate, relationship and join inputs.
- **Identities:** the before and proposed revision, fingerprint, table and API
  identities.
- **Rebuild verdict:** `unnecessary`, `required` or `blocked`.
- **Checklist:** `application_steps`, which have not run.

**Rules.**
- **Package ID changes:** they always bump the model revision, so table and API
  identities change.
- **Blockers:** removing a selected field, or changing a Numeric scale,
  optionality, an enum or a ContractId target, blocks the bundle.
- **No guessed renames:** renames are never guessed.
- **Relationships:** links into a refreshed target are flagged for review. Their
  pins never change silently.
- **Bundle contents:** the compatibility report, the impact text, models, SQL,
  GraphQL, TypeScript, an upgrade input and a hashed manifest.

**`sync evaluate`.**
- **What it does:** runs projection expressions on 1–32 explicit before and after
  samples (up to 64 KiB) and reports `projected_values_equal`.
- **What it doesn't do:** run Daml choices. A passing sample doesn't prove the
  upgrade is valid.

## Reviewed package handoff

An additive handoff keeps the source generation, the checkpoint and all history.
The old packages stay selected, and old contracts aren't migrated.

**1. Prepare.**
- **Old worker:** stop it at a completed poll.
- **New runtime:** copy `old-runtime.json` to `new-runtime.json` and **add** the
  new template triples, keeping the old ones.
- **New models:** write new model revisions for the added templates.
- **Destination:** run `storage init` (migrations 007–008).
- **Register and install:** register and install the targets. The old bindings
  stay active.

```bash
cantongrid upgrade plan .local/old-runtime.json .local/new-runtime.json \
  application.dar .local/new-models.json app > .local/upgrade.json
```

Review the manifest: scope, checkpoint, artifact hash, fingerprints and expected
revisions. Keep it private.

**2. Stage and publish.** Every command takes the same five arguments: `upgrade.json`,
`old-runtime.json`, `new-runtime.json`, the DAR and `new-models.json`.

```bash
cantongrid upgrade start    <5 args>
cantongrid upgrade step     <5 args>     # repeat while phase=snapshot
cantongrid upgrade status   <5 args>
cantongrid upgrade activate <5 args>     # at phase=ready; then step until complete
```

- **Snapshot:** baseline pages of up to 64 contracts are staged at the retained
  checkpoint while old reads continue.
  - **Stale checkpoint:** if the old worker moves the checkpoint, the job fails.
    Cancel it and plan again.
- **Publishing:** reads and ingestion on this stream are unavailable while pages
  commit. The last page switches the selection and the bindings together.
  - **Restarts:** they resume from stored progress.
- **After completion:** run with `new-runtime.json`. Polling resumes from the same
  checkpoint, and old cursors become invalid.
  - **Cleanup:** keep running `step` until `retained_pages` is 0.
- **Small snapshots** (up to 64 contracts and 1 MiB): `upgrade apply <5 args>`
  does it all in one transaction.

**3. Failure and rollback.**
- **Uncertain commit:** run `status` and resume. Repeating `activate` or `apply` is
  safe. Never reset the checkpoint.
- **Before completion:** run `upgrade cancel`, then `step` until `cancelled`.
  - **What cleanup removes:** only unpublished rows.
  - **Reads:** they resume on the old prefix.
- **After completion:** keep the wider selection. Roll back a model with a staged
  rebuild (`models rebuild start ... app markets 1 --replace 2`). Never restore the
  old runtime.

**Limits.**
- **Jobs:** one active job per stream.
- **Size:** each job is capped at 4,096 pages or 64 MiB, and each page at 64
  contracts and 1 MiB.
- **Models:** 16 active aliases.
- **Testing:** owned PostgreSQL tests cover 129 contracts, restarts, stale workers
  and cancellation. That's synthetic, not a real Canton upgrade.
