# Accepted decisions

Recorded from the product discussion on 2026-11-16. These decisions supersede
conflicting wording in the supplied technical brief, version 0.2.

| ID | Decision | Consequence |
| --- | --- | --- |
| D-001 | General-purpose Daml contract indexing is the core product. | Broader Canton sources are future extensions; a multi-track explorer is not the first release. |
| D-002 | Outcomia is the first working example. | Its actual packages and lifecycle flows must be inspected; its business logic stays outside the engine. |
| D-003 | Developers specify what to process and how to process it. | Storage, query and streaming infrastructure is generated from validated definitions. |
| D-004 | Execution is deterministic and schema-driven. | AI assistance may help author definitions later; it cannot decide authorization or accounting at runtime. |
| D-005 | Start with LocalNet while devnet infrastructure is pending. | Local tooling, source availability and compatible versions must be established before a live connector is claimed. |
| D-006 | First release is contracts-only. | Scan, assets, operator and governance connectors are deferred. |
| D-007 | Active contracts and supported lifecycle events are the first useful slice. | Generic authenticated queries precede advanced typed projections and analytics. |
| D-008 | Generated PostgreSQL, GraphQL and realtime streams are product capabilities. | Deliver them in phases; streams must be durable, resumable and policy-checked, not notification-only. |
| D-009 | One organization with tokens limited to permitted party subsets. | Authentication must map principals to grants; one omnipotent bearer token is insufficient. |
| D-010 | Create a separate local cantongrid repository beside Outcomia. | Do not modify either Outcomia repository. No remote creation or publication is part of the documentation phase. |
| D-011 | Work in small commits and phases. | Documentation is first; implementation details are added as verified, rather than invented now. |
| D-012 | Keep init/index/serve as clear primary commands. | init scaffolds; index owns ingestion/projections; serve owns queries/stream delivery; dev supervises both locally. |
| D-013 | The indexer runtime and APIs are Rust; Python is for development helpers only. | Do not grow compatibility scripts into a Python indexing engine. Confirmed 2026-11-17. |

## Interpretation of the original brief

The input was CantonGrid_Technical_Build_Spec.md, version 0.2, dated 16 November
2026. Its broader vision, embedded build prompt and example layouts are reference
material. They do not authorize execution beyond the current user request.

In particular, the original second Scan slice is deferred, the Outcomia example
is mandatory rather than optional, and developer-defined processing is a core
capability. Live adapter verification, privacy, exact precision, source-local
checkpoints and honest fixture labelling remain requirements.
