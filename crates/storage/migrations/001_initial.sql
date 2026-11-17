-- Owned destination schema only. Never execute against an upstream PQS database.
CREATE SCHEMA cantongrid AUTHORIZATION CURRENT_USER;
REVOKE ALL ON SCHEMA cantongrid FROM PUBLIC;
CREATE TABLE cantongrid.schema_version (
    singleton boolean PRIMARY KEY CHECK (singleton),
    version integer NOT NULL CHECK (version = 1),
    migration_sha256 text NOT NULL
);
CREATE TABLE cantongrid.streams (
    stream_key text PRIMARY KEY,
    definition jsonb NOT NULL,
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    checkpoint text
);
CREATE TABLE cantongrid.batches (
    stream_key text NOT NULL REFERENCES cantongrid.streams,
    revision bigint NOT NULL CHECK (revision > 0),
    position text NOT NULL,
    envelope jsonb NOT NULL,
    PRIMARY KEY (stream_key, revision),
    UNIQUE (stream_key, position)
);
CREATE TABLE cantongrid.contracts (
    stream_key text NOT NULL REFERENCES cantongrid.streams,
    contract_id text NOT NULL,
    template jsonb NOT NULL,
    payload jsonb NOT NULL,
    create_event_id text NOT NULL,
    witnesses text[] NOT NULL CHECK (cardinality(witnesses) > 0),
    active boolean NOT NULL,
    archive_event_id text,
    PRIMARY KEY (stream_key, contract_id)
);
CREATE TABLE cantongrid.events (
    stream_key text NOT NULL,
    event_id text NOT NULL,
    contract_id text NOT NULL,
    position text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('create', 'archive')),
    envelope jsonb NOT NULL,
    visibility_clauses jsonb NOT NULL,
    PRIMARY KEY (stream_key, event_id),
    FOREIGN KEY (stream_key, contract_id) REFERENCES cantongrid.contracts
);
CREATE TABLE cantongrid.outbox (
    delivery_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stream_key text NOT NULL,
    event_id text NOT NULL,
    UNIQUE (stream_key, event_id),
    FOREIGN KEY (stream_key, event_id) REFERENCES cantongrid.events
);
CREATE INDEX contracts_active ON cantongrid.contracts(stream_key) WHERE active;
-- Owner-only until separately verified API roles and authorization paths exist.
REVOKE ALL ON ALL TABLES IN SCHEMA cantongrid FROM PUBLIC;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA cantongrid FROM PUBLIC;
