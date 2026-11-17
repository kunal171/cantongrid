-- Only aggregate counts leave this read-only transaction. Identity includes interface projections.
WITH baseline AS (
    SELECT contract_id, template_fqn, payload_type::text, creation_package_id
    FROM public.active(NULL,$1)
), added AS (
    SELECT contract_id, template_fqn, payload_type::text, creation_package_id
    FROM public.creates(NULL,$1,$2)
    WHERE created_at_offset > $1 AND NOT divulged_only
), removed AS (
    SELECT contract_id, template_fqn, payload_type::text, creation_package_id
    FROM public.archives(NULL,$1,$2)
    WHERE archived_at_offset > $1 AND NOT divulged_only
), expected AS (
    (SELECT * FROM baseline UNION SELECT * FROM added) EXCEPT SELECT * FROM removed
), observed AS (
    SELECT contract_id, template_fqn, payload_type::text, creation_package_id
    FROM public.active(NULL,$2)
)
SELECT (SELECT count(*) FROM baseline), (SELECT count(*) FROM added),
       (SELECT count(*) FROM removed), (SELECT count(*) FROM observed),
       (SELECT count(*) FROM (SELECT * FROM expected EXCEPT SELECT * FROM observed) missing),
       (SELECT count(*) FROM (SELECT * FROM observed EXCEPT SELECT * FROM expected) unexpected);
