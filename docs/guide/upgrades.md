# Upgrade selected packages

Source-only **0.4.0-alpha.2** adds reviewed additive package handoffs. It keeps
one source generation, its checkpoint and existing contracts/history. Old packages
remain selected: uploading a new package does not migrate old contracts.

This command consumes explicit model JSON. Automatic DAR-to-model synchronization
is separate work in [#24](https://github.com/kunal171/cantongrid/issues/24).

## Prepare and review

1. Stop the old indexing worker at a completed polling boundary. Keep its runtime
   file as `.local/old-runtime.json`.
2. Copy it to `.local/new-runtime.json` and **add** the new exact template triples.
   Retain the old templates, physical source, parties, network and generation.
3. Prepare `.local/new-models.json` with immutable new revisions selecting the added
   templates. Review processing expressions and financial scales. Build the new DAR
   separately; the upgrade command never uploads, vets or transacts on the Ledger.
4. Stop older destination binaries and run explicit destination initialization for
   migrations **007–008**. Register/install targets before planning; these operations
   leave the old bindings active.

```bash
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid models register .local/new-models.json --namespace app \
  --database-env CANTONGRID_DESTINATION_URL
cantongrid models install app markets 2 --database-env CANTONGRID_DESTINATION_URL
cantongrid upgrade plan .local/old-runtime.json .local/new-runtime.json \
  application.dar .local/new-models.json app > .local/upgrade.json
```

Install each target in a multi-model catalog. Review the emitted manifest: source
scope, retained checkpoint, artifact hash, installed model fingerprints and expected
previous revisions. Keep it private; it contains source/party configuration.
Planning reads authorized source/package metadata and owned installation metadata;
it performs no DDL or activation.

A package must exist on the configured participant and its added templates must
exist in the verified DAR. The source reader remains SELECT-only. Current source
rights and readiness are checked again for each command.

## Stage and publish

Every operation takes the same explicit input files:

```bash
cantongrid upgrade start .local/upgrade.json .local/old-runtime.json \
  .local/new-runtime.json application.dar .local/new-models.json
cantongrid upgrade step .local/upgrade.json .local/old-runtime.json \
  .local/new-runtime.json application.dar .local/new-models.json
cantongrid upgrade status .local/upgrade.json .local/old-runtime.json \
  .local/new-runtime.json application.dar .local/new-models.json
```

Repeat `step` while `phase` is `snapshot`. PQS reads use the runtime
`page_size` (1–8, default 8); lower it for large payloads. Storage accepts at most
64 active contracts in a private page at the retained checkpoint, never at a guessed latest
head. Staging commits its source cursor and page together. Old API reads continue
against the complete old selection. If an old worker moves the checkpoint, the
upgrade fails as stale; cancel and replan rather than rewinding progress.

When `phase` is `ready`, review progress and run `upgrade activate` with the same
five arguments. Then repeat `upgrade step` until `phase` is `complete`.

During `publishing`, this stream's API reads and ingestion return unavailable.
Each bounded publication page commits behind that gate. The final page changes
all reviewed model bindings and the selection together, then removes the gate.
A restart resumes from stored progress; it does not repeat previously applied
baseline contracts. Source history/pruning and current rights are rechecked before
publication; missing coverage stops the operation without skipping ahead.

Start indexing and serving with `.local/new-runtime.json`. Forward polling resumes
from the unchanged checkpoint across both old and new packages. Existing cursors
are invalidated by the changed plan/model identity. API history coverage reports a
conservative start at the latest added-template baseline; earlier retained events
remain stored. Continue `step` after completion until `retained_pages` is zero to
remove private staging pages without deleting canonical history or model rows.

For snapshots of at most 64 contracts/1 MiB, `upgrade apply` with the same five
arguments is a single-transaction alternative to start/step/activate. It refuses
larger snapshots before mutation. Use one path per manifest.

## Failure and rollback

- After a timeout or uncertain commit, run `status` and resume. Repeat `activate`
  or `apply` safely; never reset the source checkpoint.
- Before completion, run `upgrade cancel` with the same five arguments, then
  `step` until `cancelled`. Cleanup removes only unpublished baseline/model rows
  and private pages. Public reads resume against the original prefix. A cancelled,
  fully cleaned job can be started again if its reviewed anchor/targets still match.
- After completion, retain the widened template selection and new-package history.
  Roll back a model/API revision through the existing staged rebuild, for example:

```bash
cantongrid models rebuild start .local/new-runtime.json rollback-markets app markets 1 --replace 2
cantongrid models rebuild step .local/new-runtime.json rollback-markets
# Repeat step until ready, then:
cantongrid models rebuild activate .local/new-runtime.json rollback-markets
```

This rebuild refreshes the old revision from current canonical contracts before
promotion. It does not remove new-package contracts or erase intervening history.
Do not restore the old runtime selection after a completed handoff.

## Bounds and evidence

One active staged job per stream; 4,096 pages or 64 MiB of staged envelopes per job;
64 contracts/1 MiB per page; 16 active model aliases; the existing 8 MiB projection
work budget applies per page. Bounds fail explicitly. Publication is intentionally
unavailable to public readers; outbox rows are not a public stream.

Owned-PostgreSQL tests cover 129 staged contracts, restart, mixed-package forward
activity, stale-worker rejection, atomic projection failure, partial-publication
cancellation and model rollback. These are synthetic source controls, not a real
Canton package deployment/upgrade acceptance run. See [verification](../verification.md).
