SELECT a.attname::text,
       pg_catalog.format_type(a.atttypid, a.atttypmod),
       a.attnotnull,
       COALESCE(
           NOT a.attisdropped AND NOT a.atthasdef
           AND a.attidentity = '' AND a.attgenerated = ''
           AND a.attislocal AND a.attinhcount = 0 AND a.attndims = 0
           AND NOT EXISTS (
               SELECT 1 FROM pg_catalog.aclexplode(a.attacl) acl
               WHERE acl.grantee <> (SELECT relowner FROM pg_catalog.pg_class WHERE oid = a.attrelid)
           )
           AND t.typnamespace = 'pg_catalog'::regnamespace AND t.typtype = 'b'
           AND a.attcollation = t.typcollation, false
       ) AS clean
FROM pg_catalog.pg_attribute a
LEFT JOIN pg_catalog.pg_type t ON t.oid = a.atttypid
WHERE a.attrelid = $1 AND a.attnum > 0
ORDER BY a.attnum
