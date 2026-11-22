# Upgrade impact analysis

Implemented in the **0.4.0-alpha.5 source checkout**, unpublished. This extends
`sync plan` and `sync bundle`; it uses the existing DAR reader and model compiler.
No network, database or ledger credentials are needed.

## Compare an upgrade

Supply the old DAR, the proposed DAR and models bound to the old package IDs:

```bash
cantongrid sync plan old.dar new.dar models.json > compatibility.json
cantongrid sync summary old.dar new.dar models.json
cantongrid sync bundle old.dar new.dar models.json reviewed-bundle
```

Check command exit status before consuming redirected files. Planning returns a
report even when `compatible` is false; a valid report does not mean the upgrade
is safe. Bundle generation exits unsuccessfully when blocked, before creating its
output directory. The output directory must not already exist.

The JSON report retains package/declaration changes and diagnostics. Its new
`impact` section has its own `format_version: 1` and contains:

| Field | Meaning |
| --- | --- |
| `models[].fields` | Projected output, source path, declared scalar, resolved old/new scalar and LF semantics, finite errors and compatibility |
| `models[].dependencies` | Filter inputs, aggregate group/measure inputs, relationship keys and selected join columns |
| `models[].before` / `proposed` | Revisions, fingerprints, SQL tables and generated row, diagnostic, aggregate, relationship and join API identities |
| `models[].rebuild` | `unnecessary`, `required` or `blocked` |
| `application_steps` | Operator review/installation/rebuild/activation checklist; no steps have run |

A changed package ID bumps its model revision even when selected types stay the
same. Generated table, GraphQL and TypeScript identities therefore change.
Unselected signature changes stay visible in the package diff without broadening
the projection. Unchanged selections/packages require no rebuild.

Selected field removal, exact Numeric scale, optionality, enum or ContractId target
changes block generation. Names are never guessed as renames. An unchanged SQL
output type does not establish compatible LF semantics.

Relationship target revisions are explicit policy. If a target refreshes, its
incoming links are marked for review, including links whose own package stayed
unchanged. Unresolved target blockers also propagate through incoming links.
CantonGrid does not silently change the pin. Manually review the new
model definitions, increment changed model/policy revisions and validate the
complete catalog before registration. A join target outside the supplied catalog
blocks complete bundle generation. Non-join external targets remain explicit
review dependencies and require installed-target checks at runtime.

Compatible bundles include `compatibility.json`, `impact.txt`, generated models,
metadata, SQL, GraphQL, TypeScript, an upgrade input and a hashed file manifest.
Artifact generation does not install or activate anything.

## Evaluate synthetic samples

Provide explicit old/new payloads in a fixture document:

```json
{
  "format_version": 1,
  "samples": [
    {
      "model": "quotes",
      "before": { "quantity": "10" },
      "after": { "quantity": "11" }
    }
  ]
}
```

Each payload must include every field required by that model. Use strings for
exact Int64/Numeric values; add all other projected paths to the example above.

```bash
cantongrid sync evaluate old.dar new.dar models.json samples.json
```

Results contain one-based ordinals, sanitized before/after evaluation findings and
`projected_values_equal`. Invalid evaluation yields an unknown comparison (`null`).
A blocked plan has no candidate evaluation: `after` and the comparison are `null`.
Filtered rows are compared as absent rows. Values, filter literals, payloads and
local paths are omitted. The command accepts 1–32 samples and at most 64 KiB;
unknown/duplicate fields and non-object payloads are rejected.

This runs projection expressions, not Daml choices or contract business rules.
Passing samples do not establish ledger upgrade validity or complete compatibility.

Use the [runnable PoC baseline](https://github.com/kunal171/cantongrid/tree/main/examples/upgrade-impact)
for verified DAR CLI checks. Selected type, dependency and blocked-bundle controls
run in the regular Rust sync tests.

## Apply separately

Review the report, regenerate clients, register/install required model revisions,
then follow [staged rebuilds](models.md) and [reviewed package upgrades](upgrades.md).
Retain previous templates and history. No sync command uploads/vets a DAR, changes
source grants, resets a checkpoint, deletes history or executes an upgrade.
