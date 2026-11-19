-- Operator-only rebuild progress. Partial target rows are not API bindings.
CREATE TABLE cantongrid.model_rebuilds (
    stream_key text NOT NULL REFERENCES cantongrid.streams(stream_key),
    job_id text NOT NULL CHECK (length(job_id) BETWEEN 1 AND 64),
    namespace text NOT NULL,
    name text NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    fingerprint text NOT NULL REFERENCES cantongrid.model_tables(fingerprint),
    template jsonb NOT NULL,
    previous_revision bigint,
    previous_fingerprint text REFERENCES cantongrid.model_tables(fingerprint),
    phase text NOT NULL CHECK (phase IN
        ('cleaning','backfill','catchup','ready','cleanup','complete','cancelling','cancelled')),
    scan_after text,
    source_revision bigint NOT NULL CHECK (source_revision > 0),
    scanned bigint NOT NULL DEFAULT 0 CHECK (scanned >= 0),
    processed_changes bigint NOT NULL DEFAULT 0 CHECK (processed_changes >= 0),
    projected_rows bigint NOT NULL DEFAULT 0 CHECK (projected_rows >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_key,job_id),
    FOREIGN KEY (namespace,name,revision)
        REFERENCES cantongrid.model_installations(namespace,name,revision),
    CHECK ((previous_revision IS NULL) = (previous_fingerprint IS NULL))
);
CREATE UNIQUE INDEX model_rebuilds_reserved_model ON cantongrid.model_rebuilds(stream_key,namespace,name)
    WHERE phase NOT IN ('complete','cancelled');
CREATE UNIQUE INDEX model_rebuilds_reserved_target ON cantongrid.model_rebuilds(stream_key,fingerprint)
    WHERE phase NOT IN ('complete','cancelled');
-- Coalesce changes by identity. Reconciliation reads the current canonical state, not old events.
CREATE TABLE cantongrid.model_rebuild_dirty (
    stream_key text NOT NULL,
    job_id text NOT NULL,
    contract_id text NOT NULL,
    PRIMARY KEY (stream_key,job_id,contract_id),
    FOREIGN KEY (stream_key,job_id) REFERENCES cantongrid.model_rebuilds(stream_key,job_id),
    FOREIGN KEY (stream_key,contract_id) REFERENCES cantongrid.contracts(stream_key,contract_id)
);
REVOKE ALL ON cantongrid.model_rebuilds,cantongrid.model_rebuild_dirty FROM PUBLIC;
