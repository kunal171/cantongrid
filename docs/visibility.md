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

## PQS stream and cache experiment

With the main PQS PostgreSQL fixture running, start the additional AB source:

```bash
python3 scripts/localnet-visibility-pqs.py up
curl --fail http://127.0.0.1:18081/readyz
python3 scripts/check-localnet-visibility-pqs.py
```

Wait for readiness before running the check. This source uses the dedicated
`cantongrid_visibility` database on the existing development PostgreSQL server,
with the tested public-schema layout. Public database connect/temp privileges
are revoked; the original one-party SQL reader was verified unable to connect.
The source's ledger principal has exactly the two owned read grants. The wildcard
filter selects within that fixture principal's scope; it is not a production
allowlist policy or authorization implementation.

The cached three records matched the source's exact numeric values and witness
sets: A-only had A, B-only had B, joint had A and B. The check held B revoked from
the AB source principal and observed a maximum 30-second window. Across two runs,
source readiness was lost after approximately 5.03 and 1.02 seconds respectively.
These are observed timings, not a promised revocation bound. A newly requested
B snapshot using the preissued token was rejected, and that principal could read
its current narrowed grant set without administrator rights.

All three cached records, including B-only, remained queryable by the store's
research owner. This is retained database data, not proof that an API caller may
still read B. Grants were restored in a finally block, and the source was
explicitly recreated with a renewed local token. Automatic stream recovery was
not asserted. Killed/interrupted tests still require checking the owned grants.

To stop the experiments, stop AB before its shared database and LocalNet:

```bash
python3 scripts/localnet-visibility-pqs.py stop
python3 scripts/localnet-pqs.py stop
python3 scripts/localnet.py stop
```

Contracts, parties, database contents and ignored credential/context files are
retained. No Outcomia deployment or wallet operations are performed.

## Failure triage and next runtime boundary

The first attempt to put the second source in another schema of the populated
PQS database failed during initial migration. That layout is not part of our
supported profile. A separate experiment then reproduced the underlying type
lookup defect with only an unrelated enum and the upstream SQL—no second writer,
ledger or CantonGrid code. See the [standalone reproduction](../repros/pqs-schema-type-collision/README.md).
The operational fixture uses a separate database; it does not patch upstream SQL.

The first Rust connector needs a bounded source-authorization lease in addition
to SQL reads: the tested read-only ledger principal can fetch its own current
rights. SQL-only access cannot establish current upstream party permissions.
Grant changes must invalidate the relevant scope generation and serving caches;
a retained contract must not be falsely marked archived merely to hide it.
Caller token grants remain a separate boundary. The lease, invalidation and
historical retention policy are design requirements, not implemented runtime guarantees.
