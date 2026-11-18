# Roadmap

| Milestone | State |
| --- | --- |
| Phases 0–2: design, compatibility research and fixture engine | Delivered, including owned PostgreSQL replay/recovery |
| Phase 3: authorized LocalNet contracts and generic REST/GraphQL | Delivered in `0.2.0-alpha.1`; snapshot-forward partial history, raw exact payloads |
| Phase 4: declarative models | Compiler, exact evaluator, filters, SQL plans and offline CLI delivered; live integration incomplete |
| Phase 5: durable streams | Planned |

## Next: finish Phase 4

1. Versioned model registry and managed schema installation.
2. Backfill, controlled promotion and recovery while preserving the previous version.
3. Projection updates/retractions in the canonical state/checkpoint transaction.
4. Caller-scoped model counts/pages and generated typed GraphQL.
5. Replay, archive, privacy and version/cursor tests for both examples.

Add joins and aggregates only after defining retraction and transitive visibility.
See [models](guide/models.md) for the current vocabulary and limits.

Phase 5 adds authenticated resumable outbox delivery, retention-gap behavior and
stream reauthorization. An outbox row or notification alone is not a durable stream.

Devnet needs separate authorized infrastructure and acceptance. Remote/TLS,
interfaces/reassignments, complete historical recovery, Scan, hosted multi-tenancy
and arbitrary code/SQL plugins are outside the current profile. Keep changes in
small commits, with evidence for the actual environment tested.
