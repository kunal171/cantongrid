# LocalNet source authorization

`cantongrid source verify-grants source-grants.json` checks the currently configured
source user. It reads no contract payloads and writes no ledger or database data.
It is a diagnostic, not a running indexer or website sign-in mechanism.

Example configuration (synthetic party, replace with an authorized existing party):

```json
{
  "endpoint": "http://127.0.0.1:3975",
  "user": "existing-read-only-user",
  "source": "local-provider",
  "network": "localnet",
  "generation": 1,
  "parties": ["existing-authorized-party"],
  "credential_env": "CANTONGRID_SOURCE_TOKEN"
}
```

Keep real configuration in ignored `.local/` with owner-only permissions. Supply
a current source token through the configured environment variable; never put it
in JSON, command arguments, Git or logs. This command does not create a user or
change permissions. The exact grants must already exist.

The supported release profile uses numeric-loopback HTTP, the default identity
provider and only the exact configured CanReadAs parties. ActAs, administrator,
wildcard or unknown rights fail closed. Remote endpoints and other identity
providers are not supported yet. A changed grant set needs an explicit configuration
and generation review, not automatic repair.

On success the command emits only verification status and the 15-second maximum
lease duration; it keeps history coverage unknown and ingestion disabled. Failures
omit credentials, parties, upstream bodies and configuration paths.

The Ledger authenticates the bearer. Unverified local JWT subject/expiry parsing
only rejects mismatches/expiry and caps lease lifetime; it never establishes
identity on its own. HTTP calls have a five-second overall budget and 64-KiB body
limit, with redirects/proxies disabled. A lease starts before network checks and
cannot outlive credential expiry. User and rights reads are not atomic; revocation
can be detected on renewal or at the latest when the lease expires.

Runtime integration uses `SourceAuthorization`, which clears old authorization
before renewal. Database reads must check the borrowed lease before and after the
read. API callers will require independent authentication and current party-subset
grants; a verified source user does not grant website visitors access.

Verified on LocalNet Canton 3.5.7: the existing one-party CanReadAs user succeeded;
a forged same-subject token failed at the real participant. This does not verify
SQL filter configuration, concurrent handoff, archive evidence or live ingestion.
