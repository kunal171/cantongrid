# Development tools

Run `python3 scripts/dev.py <command>` from the repository root. These Python
helpers manage fixtures and checks; the Rust CLI implements indexing and APIs.
Local generated configuration, tokens and reports stay ignored under `.local/`.

| Command | Effect |
| --- | --- |
| `localnet prepare` | Fetch pinned Quickstart checkout |
| `localnet up / status / logs / images / stop` | Manage the isolated network; stop retains volumes |
| `pqs up / status / stop` | Manage provider source and expiring local credential |
| `pqs reader` | Provision SELECT-only reader in the fixture database |
| `pqs <action> --visibility` | Use the separate AB fixture source/database |
| `storage up / status / stop` | Manage the owned PostgreSQL destination |
| `validate-dar <existing.dar>` | Validate without uploading/vetting |
| `visibility setup` | Upload fixture DAR, provision owned parties/users and seed three records |
| `security install / check [area]` | Install pinned dev tools; audit/dependencies/secrets/workflows |
| `packages verify [--directory <path>]` | Inspect clean source archives after a workspace publish dry run; no upload |
| `release check [--tag <tag>]` | Check version/license/changelog metadata; no publication |

## Checks and side effects

| `check` subcommand | Prerequisite / behavior |
| --- | --- |
| `resources` | Compose configuration only; verify log rotation |
| `ledger` | Existing LocalNet; authenticated readiness/version/ledger reads |
| `pqs [--at-offset <offset>]` | Provider source; current rights, health and SQL consistency |
| `reader` | Reader role; rolled-back write/DDL/escalation denial controls |
| `exercises` | Existing schema041 database/reader; bounded read-only catalog inspection only, no event rows |
| `exercises --fixture-tests [--dpm <path>]` | SDK 3.7.1; build generic fixture and test on IDE ledger, no external Ledger |
| `visibility` | Seeded fixture; temporarily revoke/restore owned AB grants |
| `visibility-pqs` | AB source; temporarily revoke/restore grants and observe cached rows |
| `pipeline` | Existing Visibility + isolated reader + initialized destination; run Rust worker/APIs, caller revocation and source stop/start recovery |
| `outcomia-pipeline --dar <dar> --allow-ledger-writes` | Explicit write-enabled LocalNet demonstration; upload existing DAR and submit isolated protocol/market lifecycle |

Build `examples/visibility` with its declared SDK 3.7.1 before `visibility setup`.
It targets LF 2.1. Both setup and revocation experiments need authorization for the
owned disposable LocalNet. Interrupted grant tests may require inspecting and
restoring the owned test user's scope; setup refuses mismatched existing rights.

For pipeline checks, build the Rust CLI and set `CANTONGRID_BIN` to its path:

```bash
CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check pipeline
```

This uses existing contracts, temporary application keys and a child API process;
it restores grants/services and stops the child. It still writes the owned
indexer destination. The Outcomia command and its separate write opt-in are in
[the example guide](../docs/guide/outcomia.md).

For shutdown order, volume retention and log limits, see [operations](../docs/guide/operations.md).
See [setup](../docs/guide/localnet-source.md), [operations](../docs/guide/operations.md)
and [verification](../docs/verification.md).
