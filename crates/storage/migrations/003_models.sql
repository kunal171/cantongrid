-- Definitions only: no projection tables, source bindings or API grants are installed here.
CREATE TABLE cantongrid.model_revisions (
    namespace text NOT NULL CHECK (namespace ~ '^[A-Za-z][A-Za-z0-9_-]{0,63}$'),
    name text NOT NULL CHECK (name ~ '^[A-Za-z][A-Za-z0-9_]{0,47}$'),
    revision bigint NOT NULL CHECK (revision BETWEEN 1 AND 4294967295),
    definition_version smallint NOT NULL CHECK (definition_version = 1),
    fingerprint text NOT NULL CHECK (fingerprint ~ '^[0-9a-f]{64}$'),
    definition jsonb NOT NULL CHECK (
        jsonb_typeof(definition) = 'object' AND octet_length(definition::text) <= 131072
    ),
    registered_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (namespace, name, revision)
);
REVOKE ALL ON cantongrid.model_revisions FROM PUBLIC;
