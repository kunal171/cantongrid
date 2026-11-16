# LocalNet verification evidence

Date: 2026-11-16. Work in progress; this records only observed evidence.

## Host and pinned inputs

- Ubuntu host: Docker Engine 29.1.3; Compose plugin 2.40.3.
- Java 17.0.21 is installed but not required by the network-only Docker workflow.
- DPM at `~/.dpm/bin/dpm` reports `1.1.1-snapshot-20261102.208.0.v13b5087`.
  Its cached open-source SDK manifest is 3.7.1.
- Upstream CN Quickstart revision: `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9`.
  Source configuration selects Splice 0.6.11 and Daml runtime 3.5.2.
- Existing host PostgreSQL uses 5432; this LocalNet publishes 5440 instead.

## Initial checks

Docker communication and Compose rendering succeeded. The resolved configuration
contains loopback-only published ports and isolated names. Images are being
obtained for the first start; runtime readiness and authenticated requests are
not yet claimed. Results will be updated after the probe completes.

No Outcomia DAR has been uploaded, no custom parties or transactions have been
submitted, and no PQS or CantonGrid API/indexer is running as part of this check.
