# Configure a remote source

No authorized remote source was available for alpha.6 development. The commands
below prepare and check a source; they do not establish TestNet acceptance or
production readiness. HTTPS controls have local certificate tests. Actual network
identity and Ledger/PQS participant pairing need confirmation from your provider.

## Obtain these inputs

Ask your node operator for:

- A supported JSON Ledger API HTTPS root, token issuer, identity-provider ID and
  a dedicated existing Ledger user with exactly the selected **CanReadAs** rights.
  Administrator, CanActAs, wildcard and extra party rights are rejected.
- The matching PQS/Scribe schema041 export, readiness HTTPS URL and a separate
  restricted SELECT-only PostgreSQL role. The destination owner must never read
  the source. Confirm the exporter's party scope, retained history, package
  availability and exact numeric/string JSON encoding.
- Exact deployed package hashes, template module/entity names and party IDs.
  Inspect the deployed DAR with `cantongrid metadata inspect app.dar`; package
  presence alone does not prove that a selected template exists.
- An approved SSH path to that PQS database. Current PostgreSQL clients accept
  numeric loopback connections only; direct remote PostgreSQL TLS is not supported.

The runtime reads contracts; it does not upload/vet DARs, create users/parties,
change rights or submit ledger transactions. Configure those separately with the
node operator. Use a fresh logical source/generation for a different participant;
never reuse LocalNet checkpoints on TestNet or MainNet.

## Secure transport and local files

Use a verified SSH host key and a tunnel bound only to loopback. For a database
on the SSH server's loopback interface, an example is:

```bash
ssh -N -L 127.0.0.1:5544:127.0.0.1:5432 \
  -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 operator@your-node
```

Choose the approved host and database port. `ExitOnForwardFailure` checks that
forwarding can be established; it does not prove the remote database is reachable.
If the database is on another host, the onward connection needs its own approved
protection. Do not disable host-key checking. See [SSH forwarding](https://man.openbsd.org/ssh)
and [forwarding options](https://man.openbsd.org/ssh_config).

Ledger and readiness endpoints must use certificate-verified HTTPS; redirects,
userinfo, query credentials and insecure TLS are rejected. Use a hostname covered
by a trusted certificate. No token should pass through an untrusted redirect.

```bash
umask 077
mkdir -p .local/testnet
cp examples/operations/runtime.testnet.json .local/testnet/runtime.json
cp examples/operations/grants.example.json .local/testnet/grants.json
chmod 600 .local/testnet/*.json
```

Replace every placeholder. Keep `network: "testnet"` and `transport: "remote_https"`.
Set `expected_issuer` and the exact identity-provider ID (`""` only for the default
provider). `source_health` must refer to the matching exporter's `/readyz`, with
healthy gRPC, JDBC pool and stream checks. Do not opt into exercise/tree-lineage
profiles until the export mode and completeness have been verified.

The named environment variables hold secrets, never the JSON files:

```text
CANTONGRID_SOURCE_TOKEN       dedicated Ledger user token
CANTONGRID_PQS_URL            host=127.0.0.1 port=5544 user=READ_ONLY_ROLE password=SECRET dbname=PQS_DATABASE
CANTONGRID_DESTINATION_URL    separate owned loopback database connection
CANTONGRID_API_KEY            independent organization bearer signing key
CANTONGRID_CURSOR_KEY         independent cursor signing key
```

Load these from your secret manager or an owner-only ignored environment file;
do not paste real values into documentation, command arguments or issue reports.
Use distinct random API/cursor keys of at least 32 bytes. Do not reuse the Ledger
JWT or its signing material. `grants.json` maps your organization principal IDs to
enabled party subsets of the verified source scope; it is independent of source
permissions. Keep API issuer/audience specific to your organization.

## Validate before running

Build alpha.6 from this source checkout; these commands are not in alpha.5.

```bash
cantongrid ops check-config .local/testnet/runtime.json --profile testnet
# Only on your separate owned destination, after a private backup:
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid storage check --database-env CANTONGRID_DESTINATION_URL
cantongrid ops preflight .local/testnet/runtime.json --profile testnet
```

`check-config` opens no sockets and does not validate credential values. It still
needs the named PQS connection variable to derive the source identity. `preflight`
performs read-only package/current-rights, readiness, restricted source schema and
bounds, and current destination checks. It neither installs a source role nor
migrates a database. Fix failures explicitly; there is no permission-widening
fallback. `actualNetworkVerified: false` is intentional: operator labels and a
working HTTPS endpoint are insufficient proof of TestNet identity or participant
pairing. Likewise, available packages are not independent template-entity proof.

After successful checks, start one worker and one API in separate supervised
processes, with the same reviewed configuration and environment:

```bash
cantongrid index .local/testnet/runtime.json
cantongrid serve .local/testnet/runtime.json
# In another terminal with that same environment:
cantongrid ops status .local/testnet/runtime.json
```

Keep the API loopback-bound. A remotely exposed API needs an operator-approved TLS
proxy and deployment/access review. Call authenticated `GET /v1/health`, then verify
scoped REST/GraphQL, temporal coverage and [durable delivery](delivery.md). Compare
source inventory at a known boundary; exercise A/B isolation, replay/ACK/restart,
revocation, source outage and recovery using authorized existing contracts. These
are pending remote acceptance checks, not results implied by preflight. Obtain
separate approval before test ledger writes or grant changes.

Save sanitized versions, selected profile, counts and pass/fail evidence. Never
publish runtime secret files or dumps. Use `--profile mainnet` only with a separate
reviewed `network: "mainnet"` configuration and explicit authorization.

Refresh expiring source credentials for the exact same user/scope and restart
worker/API processes with the new environment. Exporting a variable in another
shell cannot update a running process. Checkpoints and durable ACKs survive a
restart; changing keys or authorization bindings invalidates old cursors.
