# Visibility fixture

A small Daml example unrelated to Outcomia. `Visibility:Record` has an owner,
observer parties, a label, an exact Numeric 10, Int64, optional text and enum.
It does not model assets, transfer money or create prediction markets.

Build with the installed DPM SDK 3.7.1:

```bash
cd examples/visibility
~/.dpm/bin/dpm build
```

The package targets LF 2.1. Generated `.daml/` files are ignored. For local deployment and authorization checks, use the [PoC setup](../../docs/guide/localnet.md#the-poc).
Recorded results are in [verification](../../docs/verification.md).
