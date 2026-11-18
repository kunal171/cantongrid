SELECT c.contype::text, c.conkey, c.conindid,
       COALESCE(
           c.convalidated AND NOT c.condeferrable AND NOT c.condeferred
           AND c.conislocal AND c.coninhcount = 0 AND c.conparentid = 0
           AND CASE c.contype
               WHEN 'p' THEN true
               WHEN 'f' THEN
                   c.confrelid = to_regclass('cantongrid.contracts')
                   AND c.confkey = ARRAY[
                       (SELECT attnum FROM pg_catalog.pg_attribute
                        WHERE attrelid = c.confrelid AND attname = 'stream_key' AND NOT attisdropped),
                       (SELECT attnum FROM pg_catalog.pg_attribute
                        WHERE attrelid = c.confrelid AND attname = 'contract_id' AND NOT attisdropped)
                   ]::smallint[]
                   AND c.confupdtype = 'a' AND c.confdeltype = 'a' AND c.confmatchtype = 's'
               ELSE false
           END, false
       ) AS valid
FROM pg_catalog.pg_constraint c WHERE c.conrelid = $1
