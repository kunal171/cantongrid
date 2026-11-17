# Documentation

These documents distinguish the current product agreement, implemented Rust
fixture workflow, LocalNet research evidence and planned live capabilities.
Source APIs and schema mappings require compatibility verification.

| Document | Purpose |
| --- | --- |
| [Local development](localnet.md) | Reproducible LocalNet setup, endpoints and read-only usage. |
| [LocalNet evidence](localnet-evidence.md) | Observed environment and verification limits. |
| [Outcomia compatibility](outcomia-compatibility.md) | Exact DAR validation evidence and example boundaries. |
| [PQS development fixture](pqs-local.md) | Scoped local source setup and operational limits. |
| [PQS compatibility](pqs-compatibility.md) | SQL behavior, source health and recovery evidence. |
| [Visibility scenario](visibility.md) | Owned A/B/AB fixture, exact values and reversible revocation. |
| [Upstream triage](upstream-triage.md) | Failure ownership, independent reproductions and contribution candidates. |
| [Product](product.md) | Developer promise, release scope and responsibility boundaries. |
| [Architecture](architecture.md) | Components, ingestion, generated storage and recovery. |
| [Processing model](processing-model.md) | Developer inputs and deterministic generated behavior. |
| [Security](security.md) | Party-scoped access, derived data and revocation requirements. |
| [Phase 3 handoff](specifications/phase3-v0.3.md) | Supplied v0.3 scope, acceptance matrix and historical baseline. |
| [CLI](cli.md) | Implemented fixture commands and planned live commands. |
| [Durable reference ingestion](storage.md) | Owned PostgreSQL transactions, replay, source inspection and limits. |
| [Fixture pipeline](fixtures.md) | Strict configuration, exact values, lifecycle replay and limits. |
| [Development tools](../scripts/README.md) | Consolidated LocalNet commands and script review. |
| [Roadmap](roadmap.md) | Phases, acceptance gates and small-commit workflow. |
| [Phase 3 source contract](phase3-source-contract.md) | Installed interfaces, SDK evaluation, retained-window evidence and live gates. |
| [Research decisions](research-decisions.md) | Unverified interfaces and implementation gates. |
| [Ecosystem comparison](competitive-landscape.md) | Active indexers, grant proposal stages, differentiated hypotheses and CantonGrid implementation snapshot. |
| [Verification](verification.md) | Required evidence and planned runtime checks. |
| [Decisions](decisions.md) | Accepted choices and changes from the original brief. |

When documents disagree, follow current user decisions, then the accepted decision
log and product contract. Implementation must report supported capabilities
explicitly; a planned capability is not a release claim.
