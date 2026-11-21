-- Bounded, exact executed-template selection in the caller's read-only source snapshot.
WITH selected AS (
 SELECT * FROM unnest($4::text[],$5::text[],$6::text[]) AS t(package_id,module,entity)
)
SELECT e.exercised_at_offset,(e.exercise_event_id).node_id,
       CASE WHEN octet_length(t.transaction_id)<=1024 THEN t.transaction_id END,
       CASE WHEN octet_length(e.contract_id)<=1024 THEN e.contract_id END,
       e.package_id,split_part(e.template_fqn,':',2),split_part(e.template_fqn,':',3),
       CASE WHEN octet_length(e.choice)<=256 THEN e.choice END,e.consuming,
       CASE WHEN octet_length(e.argument::text)<=65536 THEN e.argument::text END,
       CASE WHEN octet_length(e.result::text)<=65536 THEN e.result::text END,
       CASE WHEN cardinality(e.controllers)<=256 AND octet_length(e.controllers::text)<=262144 THEN e.controllers END,
       CASE WHEN cardinality(e.witnesses)<=256 AND octet_length(e.witnesses::text)<=262144 THEN e.witnesses END,
       (extract(epoch FROM e.exercised_effective_at)*1000000)::bigint,
       e.last_descendant_node_id,(e.exercise_event_id).offset,e.redaction_id,
       e.choice_fqn=e.template_fqn||':'||e.choice AS direct_template_choice
FROM public.exercises(NULL,$1,$2) e
JOIN selected s ON e.package_id=s.package_id
 AND split_part(e.template_fqn,':',2)=s.module AND split_part(e.template_fqn,':',3)=s.entity
LEFT JOIN public.__transactions t ON t.ix=e.exercised_at_ix
WHERE e.exercised_at_offset>$1 AND e.witnesses && $3::text[]
ORDER BY e.exercised_at_offset,(e.exercise_event_id).node_id LIMIT 65;
