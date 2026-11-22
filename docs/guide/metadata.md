# Offline DAR metadata

Introduced in **0.4.0-alpha.2**; included in the coordinated alpha.4 release.
The command reads a local DAR and writes JSON to stdout:

```bash
cargo build -p cantongrid-cli --locked
mkdir -p .local/metadata
target/debug/cantongrid metadata inspect examples/poc/.daml/dist/cantongrid-poc-0.1.0.dar \
  > .local/metadata/poc.json
```

Build the DAR first using [the PoC guide](poc.md). Inspection needs no network,
Ledger credentials, database or running LocalNet. The CLI does not write files
itself; shell redirection creates the output file and can leave it empty on error.
Check the exit status before consuming it.

## Output

`format_version: 1` identifies the metadata schema. `artifact_sha256` hashes the
entire DAR, while `main_package_id` identifies its main DALF payload. Each package
includes its verified ID, name, version, LF version and optional upgrade predecessor.

The signature surface contains:

- Qualified record, variant, enum and interface declarations; ordered fields and parameters.
- Type aliases, parameter kinds, optionals/lists/maps, exact Numeric scale naturals
  and opaque ContractId targets.
- Templates, optional key types, consuming flags and choice argument/result types.
- Interface views, methods, choices, requirements and template implementations.
- External `type_dependencies` used by signatures, resolved within the DAR.

A type identity is `{package_id, module, name}`. Recursive records remain references;
Numeric 10 remains a `numeric` builtin with a `nat` argument of 10. No floating point
conversion or guessed SQL mapping occurs. Packages and declarations are sorted;
field, constructor and parameter order is retained. Repacking the same packages
can change `artifact_sha256` without changing their signatures.

## Supported format and bounds

Only **LF 2.1, patch 0** is accepted. LF 1, dev and other versions fail explicitly.
This is signature extraction, not complete LF execution or kind validation:
expressions, values, controllers and signatory calculations are outside the output.
`type_dependencies` is not a complete executable dependency inventory.

| Resource | Maximum |
| --- | --- |
| DAR input / JSON output | 32 MiB each |
| ZIP entries / DALF packages | 1,024 / 256 |
| Expanded entry / all entries | 8 MiB / 64 MiB |
| Manifest | 256 KiB |
| Declaration string / dotted-name segments | 4,096 bytes / 128 |
| Per-package strings / interned types / modules | 250,000 / 50,000 / 4,096 |
| Traversal depth / work nodes | 64 / 250,000 across the DAR |
| Estimated expanded signature allocation | 64 MiB |

Encrypted, unsafe or duplicate paths, inconsistent ZIP headers, CRC failures,
ambiguous manifests, missing packages, hash mismatches, invalid interning and
dangling signature references are rejected. Only stored/DEFLATE entries are read.
Limits are fixed in this version; errors omit untrusted archive contents and paths.

## After a contract update

Run inspection against the new DAR to obtain verified package IDs and signatures.
[Upgrade impact analysis](upgrade-impact.md) extends `sync plan` with dependency,
API and rebuild reports. `sync bundle` generates reviewed model/SQL/GraphQL/TS
artifacts; `sync evaluate` checks explicitly supplied local before/after samples.
The M6 additions ship in alpha.5.

Inspection and sync do not apply SQL, upload/vet DARs or activate ingestion.
[Reviewed upgrades](upgrades.md) remain a separate source-authorized operation.
Existing `models plan`, `models graphql` and `models typescript` also continue to
accept developer-authored model JSON directly.
