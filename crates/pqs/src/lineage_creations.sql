-- Metadata for proof only. Include transient/divulged nodes without materializing their contracts.
WITH selected AS (
 SELECT * FROM unnest($4::text[],$5::text[],$6::text[]) AS t(package_id,module,entity)
)
SELECT c.created_at_offset,(c.create_event_id).node_id,
 CASE WHEN octet_length(t.transaction_id)<=1024 THEN t.transaction_id END,
 CASE WHEN octet_length(c.contract_id)<=1024 THEN c.contract_id END,
 c.creation_package_id,split_part(c.template_fqn,':',2),split_part(c.template_fqn,':',3),
 CASE WHEN cardinality(c.witnesses)<=256 AND octet_length(c.witnesses::text)<=262144 THEN c.witnesses END,
 c.divulged_only,(c.create_event_id).offset,c.redaction_id
FROM public.creates(NULL,$1,$2) c
JOIN selected s ON c.creation_package_id=s.package_id
 AND split_part(c.template_fqn,':',2)=s.module AND split_part(c.template_fqn,':',3)=s.entity
LEFT JOIN public.__transactions t ON t.ix=c.created_at_ix
WHERE c.created_at_offset>$1 AND c.payload_type::text='template' AND c.witnesses && $3::text[]
ORDER BY c.created_at_offset,(c.create_event_id).node_id LIMIT 65;
