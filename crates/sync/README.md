# cantongrid-sync

This README describes CantonGrid 0.4.0-alpha.6.
Offline schema comparison and generated model bundles from verified DAR metadata
and explicit developer projections. Reports trace selected LF fields through filters,
aggregates, relationship keys and joins, with exact generated API/table identities
and rebuild requirements. Incompatible changes and target revision drift block
bundle generation; no rename or policy update is inferred.

`SyncPlan::evaluate` compares bounded operator-supplied before/after projection
samples and returns sanitized findings without values. `impact_summary` provides
a short review alongside the machine-readable report.

See the [upgrade impact guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/upgrade-impact.md)
and [example](https://github.com/kunal171/cantongrid/tree/main/examples/upgrade-impact).

It does not change databases, source grants
or installed revisions. Part of the coordinated CantonGrid workspace.

Licensed under Apache-2.0; see LICENSE and NOTICE.
