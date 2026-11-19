-- Disposable local destination; never run inside the PQS source service.
-- The destination owner can manage only this database; it is distinct from every PQS reader.
CREATE ROLE cantongrid_owner LOGIN PASSWORD 'local-cantongrid-only'
  NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE DATABASE cantongrid OWNER cantongrid_owner;
REVOKE ALL ON DATABASE cantongrid FROM PUBLIC;
\connect cantongrid
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE TEMPORARY ON DATABASE cantongrid FROM PUBLIC;
