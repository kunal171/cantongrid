# Local Canton development

The first CantonGrid environment uses the official CN Quickstart's **network-only
LocalNet**. The executable helpers here manage development infrastructure; they
are not the planned `cantongrid` product CLI. See [LocalNet evidence](localnet-evidence.md)
for the actual verification state.

## Reproduce this checkout

Prerequisites: Git, Python 3, Docker Engine and the Docker Compose plugin, with
permission to use Docker. Check `docker version` and `docker compose version`.
Allow several GB of downloads and disk space on the first start. The upstream
resource configuration limits individual services; the full stack still requires
substantial free memory. Do not build the Quickstart demo for this workflow.

From the CantonGrid repository:

```bash
python3 scripts/localnet.py prepare
python3 scripts/localnet.py up
python3 scripts/localnet.py status
python3 scripts/probe-localnet.py
```

The first command obtains the upstream checkout under ignored `.local/`. It pins
revision `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9`, whose `.env` selects Splice
`0.6.11`. Subsequent commands refuse a changed revision or dirty checkout.
`up` launches detached containers; container creation alone is not readiness.
Retry the probe after initialization. A failed probe exits nonzero.

Our helper resolves upstream Compose configuration, then isolates container names,
network, volumes and host bindings under `cantongrid-localnet`. The generated
configuration stays in ignored `.local/` with owner-only file permissions.
Upstream services provision their own demo network state as part of startup;
these helpers submit no custom ledger commands or Outcomia fixtures.

## Endpoints in this configuration

All published ports bind to `127.0.0.1`. These are deliberately local development
credentials and services; they are not the CantonGrid application API.

| Service | App User | App Provider | Super Validator |
| --- | --- | --- | --- |
| JSON Ledger API | `http://127.0.0.1:2975` | `http://127.0.0.1:3975` | `http://127.0.0.1:4975` |
| gRPC Ledger API | `127.0.0.1:2901` | `127.0.0.1:3901` | `127.0.0.1:4901` |
| Validator readiness | `http://127.0.0.1:2903/api/validator/readyz` | `http://127.0.0.1:3903/api/validator/readyz` | `http://127.0.0.1:4903/api/validator/readyz` |
| Wallet UI | `http://wallet.localhost:12000` | `http://wallet.localhost:13000` | `http://wallet.localhost:14000` |

Scan is at `http://scan.localhost:14000`; SV UI is at
`http://sv.localhost:14000`. The database publishes `127.0.0.1:5440`, preserving
this machine's existing PostgreSQL on 5432. UI host ports differ from upstream
defaults to avoid common frontend development ports. API ports remain upstream
defaults; another stack on those ports must be stopped before starting this one.
Do not run upstream `make start` alongside this stack.

## Authenticated read-only use

`probe-localnet.py` checks validator readiness and JSON API readiness, verifies
that a party-list request without a token is rejected, and then reads version,
ledger end and one party-list page using a short-lived development JWT. It keeps
the token in memory and prints only counts and nonsecret version/offset data.
Its fixed URLs cannot accidentally target a shared network.

The upstream development token uses HS256 with the published `unsafe` secret,
audience `https://canton.network.global` and subject `ledger-api-user`. An admin
user still needs party read/act grants for contract access. A successful version
or party-list request does not prove authorized contract ingestion. CantonGrid
must later use explicit source scopes and independently scoped API principals.

Use the verified JSON API specification when adding active-contract or update
requests; do not invent request shapes. No DAR upload, party allocation, command
submission or Outcomia deployment is included in this workflow.

## Inspect and stop

```bash
python3 scripts/localnet.py logs
python3 scripts/localnet.py images
python3 scripts/localnet.py stop
```

`stop` removes this project's containers and network, retaining named data
volumes. The next `up` reuses them. There is deliberately no destructive reset
shortcut. Avoid global Docker cleanup commands. If startup fails, inspect status
and service logs; a source auth or readiness failure must not be treated as an
empty ledger.

## Sandbox alternative and PQS boundary

Official DPM Sandbox is the lighter single-participant option. It is useful for
focused contract/API tests and does not supply LocalNet's wallet ecosystem. The
installed DPM is outside the shell PATH on this machine; it can be invoked at
`~/.dpm/bin/dpm`. The cached SDK is 3.7.1, while the pinned Quickstart declares
Daml runtime 3.5.2. Do not assume those versions or compiled DARs are interchangeable.

Network-only LocalNet does **not** start the Quickstart's optional PQS module.
PQS selection, privileges, history, snapshot handoff and version compatibility
remain gates before a CantonGrid connector is built. LocalNet's PostgreSQL is
upstream storage; CantonGrid must use a separately owned destination database.

## Primary references

- [Official LocalNet guide](https://docs.canton.network/sdks-tools/development-tools/localnet).
- [Pinned Quickstart source](https://github.com/digital-asset/cn-quickstart/tree/8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9).
- [Official Sandbox guide](https://docs.canton.network/sdks-tools/development-tools/sandbox).
- [Official Ledger API reference](https://docs.canton.network/sdks-tools/api-reference/ledger-api).
- [Current JSON Ledger OpenAPI](https://docs.canton.network/openapi/json-ledger-api/openapi.yaml).

The downloaded current OpenAPI identifies itself as 3.5.19; it is reference
material, not proof that all its methods exist in the pinned LocalNet image.
