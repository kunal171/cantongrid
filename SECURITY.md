# Security

CantonGrid is pre-release software. Fixture tests and source inspection are not a
production security certification. The current source/transport restrictions and
open live-ingestion gates are documented in [storage](docs/storage.md) and the
[security model](docs/security.md).

## Reporting

Use [GitHub private vulnerability reporting](https://github.com/kunal171/cantongrid/security/advisories/new)
when available. Include affected commit/version, impact and a minimal synthetic
reproduction. Never include keys, bearer tokens or private ledger payloads.
If private reporting is unavailable, open an issue asking for a private contact
without disclosing vulnerability details. Do not publish an exploit or personal data.

## Security changes

Authorization, migration, source-scope, cursor and dependency changes require
focused tests and review. Do not bypass source/caller checks to recover availability.
Any advisory/scanner exception must identify its scope, reason and review deadline;
blanket exclusions for source code or history are not acceptable.
