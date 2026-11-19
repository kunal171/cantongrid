-- Check every owned destination object, including column grants invisible in relacl.
SELECT NOT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_namespace n
    CROSS JOIN LATERAL pg_catalog.aclexplode(
        COALESCE(n.nspacl, pg_catalog.acldefault('n', n.nspowner))
    ) acl
    WHERE n.nspname = 'cantongrid' AND acl.grantee <> n.nspowner
) AND NOT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_class c
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'cantongrid'
      AND c.relkind IN ('r', 'p', 'S', 'v', 'm', 'f')
      AND (
          c.relowner <> n.nspowner
          OR EXISTS (
              SELECT 1 FROM pg_catalog.aclexplode(c.relacl) acl
              WHERE acl.grantee <> c.relowner
          )
          OR EXISTS (
              SELECT 1
              FROM pg_catalog.pg_attribute a
              CROSS JOIN LATERAL pg_catalog.aclexplode(a.attacl) acl
              WHERE a.attrelid = c.oid AND acl.grantee <> c.relowner
          )
      )
)
