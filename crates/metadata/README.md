# cantongrid-metadata

This README describes CantonGrid 0.4.0-alpha.7.
Offline, bounded DAR/DALF signature extraction for CantonGrid. Metadata does not
authorize access, prove deployment, validate contract execution or apply migrations.

```toml
[dependencies]
cantongrid-metadata = "=0.4.0-alpha.7"
```

```rust,no_run
let metadata = cantongrid_metadata::inspect_file(std::path::Path::new("application.dar"))?;
println!("{}", metadata.to_json()?);
# Ok::<(), cantongrid_metadata::MetadataError>(())
```

Or run `cantongrid metadata inspect application.dar` using the CLI.
Only LF 2.1 is supported. Qualified records, aliases, templates/choices and
interfaces retain exact package identities and type arguments. The parser checks
ZIP safety, payload hashes, signature references and fixed resource bounds.

See the [metadata guide](https://github.com/kunal171/cantongrid/blob/feat/dar-metadata/docs/guide/metadata.md)
for the output contract, limits and unsupported operations.
Install the matching CLI version for the metadata inspection commands.

Licensed under Apache-2.0; see LICENSE and NOTICE.
