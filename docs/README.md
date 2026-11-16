# Documentation

These documents capture the current product agreement. They are design contracts,
not evidence that a runtime exists. Specific source APIs and schema mappings will
be added only after compatibility verification.

| Document | Purpose |
| --- | --- |
| [Product](product.md) | Developer promise, release scope and responsibility boundaries. |
| [Architecture](architecture.md) | Components, ingestion, generated storage and recovery. |
| [Processing model](processing-model.md) | Developer inputs and deterministic generated behavior. |
| [Security](security.md) | Party-scoped access, derived data and revocation requirements. |
| [CLI](cli.md) | Planned commands and operator/developer workflow. |
| [Roadmap](roadmap.md) | Phases, acceptance gates and small-commit workflow. |
| [Decisions](decisions.md) | Accepted choices and changes from the original brief. |

When documents disagree, follow current user decisions, then the accepted decision
log and product contract. Implementation must report supported capabilities
explicitly; a planned capability is not a release claim.
