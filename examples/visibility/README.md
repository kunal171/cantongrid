# Visibility fixture

A small Daml example unrelated to Outcomia. `Visibility:Record` has an owner,
observer parties, a label, an exact Numeric 10, Int64, optional text and enum.
It does not model assets, transfer money or create prediction markets.

Build with the installed DPM SDK 3.7.1:

```bash
cd examples/visibility
~/.dpm/bin/dpm build
```

The package targets LF 2.1. Generated `.daml/` files are ignored. The source
fixture alone does not establish live authorization behavior; local deployment
and scenario evidence are recorded separately as tests are added.
