-- Operator setup for this isolated development database, never an engine migration.
BEGIN;
DO $$
BEGIN
  IF current_database() <> 'pqs' OR current_user <> 'pqs_owner' THEN
    RAISE EXCEPTION 'Reader setup requires the owned PQS fixture database';
  END IF;
  IF to_regprocedure('public.active(text,bigint)') IS NULL THEN
    RAISE EXCEPTION 'Initialize PQS schema before reader setup';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_proc WHERE pronamespace = 'public'::regnamespace AND prosecdef) THEN
    RAISE EXCEPTION 'Review security-definer functions before granting reader access';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'cantongrid_pqs_reader') THEN
    CREATE ROLE cantongrid_pqs_reader LOGIN PASSWORD 'local-pqs-reader'
      NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS;
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'cantongrid_pqs_reader'
      AND (rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls))
    OR EXISTS (SELECT 1 FROM pg_auth_members WHERE member = 'cantongrid_pqs_reader'::regrole)
    OR EXISTS (SELECT 1 FROM pg_class WHERE relowner = 'cantongrid_pqs_reader'::regrole)
    OR EXISTS (SELECT 1 FROM pg_proc WHERE proowner = 'cantongrid_pqs_reader'::regrole)
    OR EXISTS (SELECT 1 FROM pg_namespace WHERE nspowner = 'cantongrid_pqs_reader'::regrole)
  THEN
    RAISE EXCEPTION 'Existing reader has unexpected privileges or ownership; no changes applied';
  END IF;
END $$;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE TEMPORARY ON DATABASE pqs FROM PUBLIC;
GRANT CONNECT ON DATABASE pqs TO cantongrid_pqs_reader;
GRANT USAGE ON SCHEMA public TO cantongrid_pqs_reader;
-- SQL access is a coarse source-storage boundary, not API caller authorization.
-- The Rust worker still checks current Ledger rights and exact configured selections.
GRANT SELECT ON ALL TABLES IN SCHEMA public TO cantongrid_pqs_reader;
ALTER DEFAULT PRIVILEGES FOR ROLE pqs_owner IN SCHEMA public
  GRANT SELECT ON TABLES TO cantongrid_pqs_reader;
-- This default is defense in depth; the rollback-only probes verify real ACL denial
-- even when a session explicitly requests a read-write transaction.
ALTER ROLE cantongrid_pqs_reader SET default_transaction_read_only = on;
DO $$
BEGIN
  IF has_schema_privilege('cantongrid_pqs_reader', 'public', 'CREATE')
    OR has_database_privilege('cantongrid_pqs_reader', 'pqs', 'CREATE,TEMP')
    OR EXISTS (SELECT 1 FROM pg_class c
      WHERE relnamespace = 'public'::regnamespace AND relkind IN ('r', 'p')
      AND has_table_privilege('cantongrid_pqs_reader', c.oid,
        'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER'))
  THEN
    RAISE EXCEPTION 'Existing reader has unexpected write privileges; no changes applied';
  END IF;
END $$;
COMMIT;
