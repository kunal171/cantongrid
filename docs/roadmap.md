# Roadmap

| Milestone | State |
| --- | --- |
| Phases 0–2: design, compatibility research and fixture engine | Delivered, including owned PostgreSQL replay/recovery |
| Phase 3: authorized LocalNet contracts and generic REST/GraphQL | Delivered in `0.2.0-alpha.1`; snapshot-forward partial history, raw exact payloads |
| Phase 4: declarative models | Compiler, registry, installation and bounded live projections delivered; typed queries pending |
| Phase 5: durable streams | Planned |

## Next: finish Phase 4

1. Caller-scoped model counts/pages and generated typed GraphQL.
2. Revision-bound cursors and privacy tests for both examples.
3. Actual LocalNet model activation/lifecycle acceptance; current projection controls use owned PostgreSQL and mock source authority.
4. Define staged rebuilds for datasets above the atomic backfill budget before supporting them.

Add joins and aggregates only after defining retraction and transitive visibility.
See [models](guide/models.md) for the current vocabulary and limits.

Phase 5 adds authenticated resumable outbox delivery, retention-gap behavior and
stream reauthorization. An outbox row or notification alone is not a durable stream.

Devnet needs separate authorized infrastructure and acceptance. Remote/TLS,
interfaces/reassignments, complete historical recovery, Scan, hosted multi-tenancy
and arbitrary code/SQL plugins are outside the current profile. Keep changes in
small commits, with evidence for the actual environment tested.
