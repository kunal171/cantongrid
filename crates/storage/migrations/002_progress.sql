-- Owned destination only. Never modify the immutable 001 migration or upstream PQS.
CREATE TABLE cantongrid.runtime_migrations (
    version integer PRIMARY KEY,
    migration_sha256 text NOT NULL
);
ALTER TABLE cantongrid.streams ADD COLUMN observed_at timestamptz NOT NULL DEFAULT now();
REVOKE ALL ON cantongrid.runtime_migrations FROM PUBLIC;
