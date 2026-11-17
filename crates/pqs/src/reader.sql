-- All temporal inputs and source-party filters stay inside one source SQL snapshot.
WITH selected AS (
    SELECT * FROM unnest($7::text[],$8::text[],$9::text[]) AS t(package_id,module,entity)
), baseline AS MATERIALIZED (
    SELECT c.* FROM public.active(NULL,$2) c
    JOIN selected t ON c.creation_package_id=t.package_id
        AND split_part(c.template_fqn,':',2)=t.module AND split_part(c.template_fqn,':',3)=t.entity
    WHERE c.payload_type::text='template' AND NOT c.divulged_only AND c.witnesses && $6::text[]
), added AS MATERIALIZED (
    SELECT c.* FROM public.creates(NULL,$2,$3) c
    JOIN selected t ON c.creation_package_id=t.package_id
        AND split_part(c.template_fqn,':',2)=t.module AND split_part(c.template_fqn,':',3)=t.entity
    WHERE NOT $1 AND c.created_at_offset>$2 AND c.payload_type::text='template'
        AND NOT c.divulged_only AND c.witnesses && $6::text[]
), visible_ids AS (
    SELECT contract_id,creation_package_id,template_fqn FROM baseline
    UNION SELECT contract_id,creation_package_id,template_fqn FROM added
), source_rows AS (
    SELECT CASE WHEN $1 THEN $2 ELSE c.created_at_offset END AS source_offset,
           c.create_event_pk AS pk,false AS archive,c.create_event_id::text AS event_id,
           c.contract_id,c.template_fqn,c.creation_package_id,c.witnesses,
           CASE WHEN octet_length(c.payload::text)<=65536 THEN c.payload::text END AS payload,
           (extract(epoch FROM c.created_effective_at)*1000000)::bigint AS ledger_time,c.created_at_offset AS origin_offset
    FROM (SELECT * FROM baseline WHERE $1 UNION ALL SELECT * FROM added) c
    UNION ALL
    SELECT a.archived_at_offset,a.archive_event_pk,true,a.archive_event_id::text,
           a.contract_id,a.template_fqn,a.creation_package_id,a.witnesses,NULL::text,
           (extract(epoch FROM a.archived_effective_at)*1000000)::bigint,a.archived_at_offset
    FROM public.archives(NULL,$2,$3) a JOIN visible_ids v USING(contract_id,creation_package_id,template_fqn)
    WHERE NOT $1 AND a.archived_at_offset>$2 AND a.payload_type::text='template' AND NOT a.divulged_only
)
SELECT * FROM source_rows WHERE CASE WHEN $1 THEN pk>$5 ELSE (source_offset,pk)>($4,$5) END
ORDER BY source_offset,pk LIMIT $10;
