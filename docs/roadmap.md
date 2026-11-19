# Roadmap

| Milestone | State |
| --- | --- |
| Phases 0–2: design, compatibility research and fixture engine | Delivered, including owned PostgreSQL replay/recovery |
| Phase 3: authorized LocalNet contracts and generic REST/GraphQL | Delivered in `0.2.0-alpha.1`; snapshot-forward partial history, raw exact payloads |
| Phase 4: declarative models | Compiler, registry, installation and bounded live projections delivered; typed GraphQL and PoC released in 0.3.0-alpha.2; alpha.4 filters/sorting/discovery verified on LocalNet |
| Phase 5: durable streams | Planned |

## Next: finish Phase 4

1. Verify the staged rebuild workflow on a larger LocalNet dataset. The unreleased
   alpha.5 implementation has owned-PostgreSQL and mock-source CLI coverage for
   resume, ingestion between pages, authorization renewal and atomic activation.
2. Extend application projections beyond the verified BinaryMarket lifecycle only
   after obtaining funded-asset/trading acceptance authorization.

The [Outcomia model check](guide/outcomia.md#localnet-demonstration) now verifies
real application choices, exact filters, private scopes, cursors and optional-field
promotion/rollback. This is bounded LocalNet acceptance, not complete Phase 4.

Add joins and aggregates only after defining retraction and transitive visibility.
See [models](guide/models.md) for the current vocabulary and limits.

Phase 5 adds authenticated resumable outbox delivery, retention-gap behavior and
stream reauthorization. An outbox row or notification alone is not a durable stream.

Devnet needs separate authorized infrastructure and acceptance. Remote/TLS,
interfaces/reassignments, complete historical recovery, Scan, hosted multi-tenancy
and arbitrary code/SQL plugins are outside the current profile. Keep changes in
small commits, with evidence for the actual environment tested.
