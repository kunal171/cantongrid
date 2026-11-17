# Local visibility scenario

This is an owned LocalNet test, not the CantonGrid Rust engine. It deploys only
`examples/visibility`, allocates two dedicated test parties and writes three
small fixture contracts. It does not deploy Outcomia or use wallet balances.

## Reproduce

Build the Daml example, start LocalNet and then run from CantonGrid:

```bash
python3 scripts/setup-localnet-visibility.py
python3 scripts/check-localnet-visibility.py
```

Setup validates/uploads/vets the fixture DAR, creates a writer restricted to the
two fixture parties, and creates three read-only users. It reuses exact owned
party hints and verified users; unexpected scope or fixture data causes failure.
Re-running setup was checked to retain exactly three records. Actual party IDs
and scope context stay in owner-only `.local/visibility/context.json`.

The check temporarily revokes B from the owned AB user and restores that grant
in a finally block. It never changes another application's user permissions.
An interrupted/killed process may require inspection of the owned AB user before
rerunning; setup refuses to silently replace mismatched grants.

## Observed Ledger API results — 2026-11-17

Tested against the running participant's own `/docs/openapi` version 3.5.7.
The unrelated fixture was built with SDK 3.7.1 targeting LF 2.1.

| Read scope | A-only | B-only | Joint (A owner, B observer) |
| --- | --- | --- | --- |
| A | Visible | Hidden | Visible |
| B | Hidden | Visible | Visible |
| AB | Visible | Visible | Visible |

A-only requests naming B, B-only requests naming A, and A-only requests naming
both parties all returned HTTP 403. After revoking B from AB, new B/AB snapshot
requests failed using both a preissued token and a fresh token. A-scoped reads
with the preissued token still worked; the independent B principal was unaffected.
The AB principal could read its updated own rights without administrator access.
After restoration, AB again saw exactly the original three contracts.

The payload round trip preserved Numeric 10 value
`12345678901234567890.1234567891` and Int64 maximum `9223372036854775807` as strings,
plus a null optional and the enum value Ready. This does not establish every
Daml type/variant encoding or all numeric scales.

## Consequences and limits

A single jointly visible contract is readable through either authorized
stakeholder. That is different from deriving a row from an A-only record and a
B-only record: a derived join requires access to both contributing inputs.
The future Rust processing layer must retain this dependency distinction.

These results cover fresh Ledger API snapshot requests, not an already open
stream, authorization lease implementation, cached API data, joins, GraphQL
resolvers or historical revocation policy. PQS/cache behavior is tested separately.

Primary references: [official JSON API tutorial](https://docs.canton.network/appdev/modules/m4-json-api-tutorial)
and the version-specific OpenAPI served by the local participant. The local spec
was inspected before constructing the fixture's request shapes.
