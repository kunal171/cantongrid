# Outcomia compatibility check

Verified locally on 2026-11-17 against the App Provider participant, Canton 3.5.7.
This is a package validation check, not an Outcomia deployment or trading test.

## Artifact evidence

The existing sibling Outcomia DAR passed `POST /v2/dars/validate`:

- File: `contracts/main/.daml/dist/outcomia-markets-0.1.0.dar` in outcomia-canton.
- SHA-256: `eef7340eba83f4f2af4865258c66c6461876bacb05b8e4dbc6c7ca74459f6f0e`.
- Main package ID: `73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a`.
- The source project declares SDK 3.7.1 and LF target 2.1. The validated DAR was
  already built; this check does not establish that it matches every current source file.

Reproduce from CantonGrid after LocalNet is ready:

```bash
python3 scripts/validate-localnet-dar.py \
  ../outcomia-canton/contracts/main/.daml/dist/outcomia-markets-0.1.0.dar
```

The helper reports the exact artifact hash and fails on rejection. It only calls
validation; it does not upload, vet packages, allocate parties or submit commands.
The main package remained absent from the participant's package inventory after
validation. Changing the DAR requires a new validation result and hash.

The [official JSON API specification](https://docs.canton.network/openapi/json-ledger-api/openapi.yaml)
describes this validation endpoint as having no persistent ledger or package effect.
The successful call verifies that endpoint on this particular LocalNet version.

## Initial source review for the example

The actual Canton source defines `Outcomia.Binary:Protocol` and
`Outcomia.Binary:BinaryMarket`. Protocol revisions carry the operator, protocol
name, admin list, market counter and fee settings. BinaryMarket includes its
operator, creator, protocol name, market ID, terms, timestamps and lifecycle state.
Changes such as opening a market consume a revision and create a successor.

Example models must preserve contract IDs and explicitly define revision/domain
identity. Operator/name alone must not be assumed globally unique across protocol
roots. `Closed` stops trading; it does not prove final resolution or settlement.
Custody, orders, positions and resolution use separate templates. A market record
must not be interpreted as proof of collateral or a user's balance.

This is a source review, not a verified metadata extraction pipeline. Do not copy
these domain rules into the generic Rust engine. Typed bindings, actual authorized
contract reads, complete lifecycle fixtures and an unrelated Daml example are
still required. No Outcomia contracts have been indexed in the PQS fixture yet.
