-- One active revision per source stream and namespaced model. These are not API grants.
CREATE TABLE cantongrid.model_bindings (
    stream_key text NOT NULL REFERENCES cantongrid.streams(stream_key),
    namespace text NOT NULL,
    name text NOT NULL,
    revision bigint NOT NULL,
    fingerprint text NOT NULL REFERENCES cantongrid.model_tables(fingerprint),
    bound_at_revision bigint NOT NULL CHECK (bound_at_revision > 0),
    bound_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_key, namespace, name),
    FOREIGN KEY (namespace, name, revision)
        REFERENCES cantongrid.model_installations(namespace, name, revision)
);
CREATE INDEX model_bindings_stream_fingerprint
    ON cantongrid.model_bindings(stream_key, fingerprint);
REVOKE ALL ON cantongrid.model_bindings FROM PUBLIC;
