-- Physical schemas and revision links only. Installation does not activate a model.
CREATE TABLE cantongrid.model_tables (
    fingerprint text PRIMARY KEY CHECK (fingerprint ~ '^[0-9a-f]{64}$'),
    table_name text NOT NULL UNIQUE CHECK (table_name ~ '^cg_model_[0-9a-f]{48}$'),
    plan_version smallint NOT NULL CHECK (plan_version = 1),
    ddl_sha256 text NOT NULL CHECK (ddl_sha256 ~ '^[0-9a-f]{64}$'),
    installed_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE cantongrid.model_installations (
    namespace text NOT NULL,
    name text NOT NULL,
    revision bigint NOT NULL,
    fingerprint text NOT NULL REFERENCES cantongrid.model_tables(fingerprint),
    installed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (namespace, name, revision),
    FOREIGN KEY (namespace, name, revision)
        REFERENCES cantongrid.model_revisions(namespace, name, revision)
);
REVOKE ALL ON cantongrid.model_tables, cantongrid.model_installations FROM PUBLIC;
