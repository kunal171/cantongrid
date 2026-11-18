SELECT COALESCE(
    relkind = 'r' AND relpersistence = 'p' AND NOT relispartition
    AND NOT relrowsecurity AND NOT relforcerowsecurity
    AND relowner = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = current_user)
    AND NOT EXISTS (
        SELECT 1 FROM pg_catalog.aclexplode(
            COALESCE(relacl, pg_catalog.acldefault('r', relowner))
        ) acl WHERE acl.grantee <> relowner
    )
    AND NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_inherits WHERE inhrelid = $1 OR inhparent = $1
    )
    AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_rewrite WHERE ev_class = $1)
    AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_policy WHERE polrelid = $1)
    AND NOT EXISTS (
        SELECT 1 FROM pg_catalog.pg_trigger
        WHERE tgrelid = $1 AND (NOT tgisinternal OR tgenabled <> 'O')
    ), false
)
FROM pg_catalog.pg_class WHERE oid = $1
