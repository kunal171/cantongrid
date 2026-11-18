SELECT (SELECT count(*) FROM pg_catalog.pg_index WHERE indrelid = $1) = 1
   AND EXISTS (
       SELECT 1 FROM pg_catalog.pg_index i
       JOIN pg_catalog.pg_class c ON c.oid = i.indexrelid
       JOIN pg_catalog.pg_am am ON am.oid = c.relam
       WHERE i.indrelid = $1 AND i.indexrelid = $2 AND am.amname = 'btree'
         AND i.indisunique AND i.indisprimary AND i.indimmediate
         AND i.indisvalid AND i.indisready AND i.indislive
         AND NOT i.indisexclusion AND i.indnkeyatts = 2 AND i.indnatts = 2
         AND i.indexprs IS NULL AND i.indpred IS NULL
         AND i.indkey::text = '1 2' AND i.indoption::text = '0 0'
         AND NOT EXISTS (
             SELECT 1 FROM unnest(i.indclass::oid[]) op
             JOIN pg_catalog.pg_opclass cls ON cls.oid = op
             WHERE NOT cls.opcdefault OR cls.opcintype <> 'pg_catalog.text'::regtype
                 OR cls.opcmethod <> c.relam
         )
         AND NOT EXISTS (
             SELECT 1 FROM unnest(i.indcollation::oid[]) collation_oid
             WHERE collation_oid <> (SELECT typcollation FROM pg_catalog.pg_type
                                     WHERE oid = 'pg_catalog.text'::regtype)
         )
   )
