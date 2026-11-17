# LocalNet verification evidence

Date: 2026-11-16. This records observed local infrastructure evidence, not
CantonGrid ingestion or Outcomia deployment acceptance.

## Host and pinned inputs

- Ubuntu host: Docker Engine 29.1.3; Compose plugin 2.40.3.
- Java 17.0.21 is installed but not required by the network-only Docker workflow.
- DPM at `~/.dpm/bin/dpm` reports `1.1.1-snapshot-20261102.208.0.v13b5087`.
  Its cached open-source SDK manifest is 3.7.1.
- Upstream CN Quickstart revision: `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9`.
  Source configuration selects Splice 0.6.11 and Daml runtime 3.5.2.
- Existing host PostgreSQL uses 5432; this LocalNet publishes 5440 instead.

## Initial checks

Docker communication and Compose rendering succeeded. All resolved published
ports use loopback, and container/network/volume names are isolated. Conflicting
shell values for image version, database port, network and test port did not
change the generated configuration. UI listener ports and wallet redirect
configuration agree.

The initial startup exposed an invalid default party hint derived from the
hyphenated Docker network name. An explicit `cantongrid-localparty-1` fixes this;
the helper now sets it independently. No source checkout modification or volume
deletion was required.

After that fix, all three validators and JSON Ledger API readiness checks passed.
The APIs report Canton **3.5.7**. Each rejected an unauthenticated party-list
request with HTTP 401, then accepted the helper's short-lived development JWT for
version, ledger end and party listing. One observed sample was:

| Participant | Version | Ledger offset | Party-list page count |
| --- | --- | --- | --- |
| App User | 3.5.7 | 30 | 6 |
| App Provider | 3.5.7 | 32 | 6 |
| Super Validator | 3.5.7 | 67 | 6 |

Offsets and counts are observations, not expected constants or history coverage
claims.

## Restart and running endpoints

The healthy stack was stopped using `localnet.py stop` and started again using
`localnet.py up`. All three participant IDs matched the private local baseline;
all baseline parties remained known; and ledger offsets did not regress. The
PostgreSQL volume retained its original creation timestamp. This verifies local
network persistence, not a CantonGrid checkpoint or contract-history replay.
The authenticated probe passed again after restart.

Wallet pages for all three validators, Scan and the SV UI returned HTTP 200 with
HTML on the documented hostnames/ports. This is HTTP availability evidence, not
browser login, wallet transfer or full UI interaction testing. Inspection of all
11 running containers confirmed that every published host port binds only to
`127.0.0.1`.

The stack is left running for subsequent integration work. The existing host
PostgreSQL on 5432 and the Outcomia repositories were not modified.

No Outcomia DAR has been uploaded, no custom parties or transactions have been
submitted, and no PQS or CantonGrid API/indexer is running as part of this check.


## Remaining integration gates

The cached `dpm pqs --version` reports PQS 3.7.0, Daml SDK 3.7.0 and document
schema 043; its help command works, but no PQS pipeline was launched. The pinned
Quickstart optional PQS module selects Scribe 3.5.4. Do not treat the cached newer
PQS or Outcomia SDK 3.7.1 as verified compatible with Canton 3.5.7.

Active-contract visibility, snapshot/update handoff, lifecycle ingestion, exact
schema mapping, scoped non-admin users and generated CantonGrid APIs remain
unverified. Admin read-only health checks do not establish these guarantees.


## Follow-up — 2026-11-17

After the interrupted host boot, the existing stack required an orderly
stop/start with its named volumes retained. All three authenticated probes
passed again. No user funds or source repositories were changed.

The [PQS fixture and SQL checks](pqs-compatibility.md) now verify Scribe 3.5.4 /
schema 041 against this participant, using a separate database and one-party
read-only ledger user. The [Outcomia artifact](outcomia-compatibility.md) passed
non-persisting DAR validation. The earlier remaining-gates section describes the
initial handoff; these linked results supersede its unverified baseline items.

No Outcomia DAR has been uploaded and no custom contract transactions were
submitted. PQS is running for the local test with a one-hour credential;
`localnet-pqs.py up` renews it. Broader source authorization and the Rust indexer
remain unimplemented/unverified as described in the compatibility results.


## Visibility fixture follow-up — 2026-11-17

Subsequent [visibility tests](visibility.md) uploaded only the unrelated
cantongrid-visibility-fixture package, allocated two owned development parties,
and created exactly three test records. The earlier no-custom-transactions
statement describes the preceding compatibility checkpoint. Outcomia remains
undeployed and wallet balances were not used.

A separate AB source/database verified witness sets, exact payload values and
revocation/cache behavior. The migration failure was [triaged independently](upstream-triage.md)
and a standalone reproduction plus local issue draft were committed. No upstream
publication or production migration patch was performed.
