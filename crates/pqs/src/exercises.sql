-- Bounded, exact executed-template selection in the caller's read-only source snapshot.
WITH selected AS (
 SELECT * FROM unnest($4::text[],$5::text[],$6::text[]) AS t(package_id,module,entity)
)
SELECT e.exercised_at_offset,(e.exercise_event_id).node_id,t.transaction_id,
       e.contract_id,e.package_id,split_part(e.template_fqn,':',2),split_part(e.template_fqn,':',3),
       e.choice,e.consuming,
       CASE WHEN octet_length(e.argument::text)<=65536 THEN e.argument::text END,
       CASE WHEN octet_length(e.result::text)<=65536 THEN e.result::text END,
       e.controllers,e.witnesses,(extract(epoch FROM e.exercised_effective_at)*1000000)::bigint,
       e.last_descendant_node_id,(e.exercise_event_id).offset,e.redaction_id
FROM public.exercises(NULL,$1,$2) e
JOIN selected s ON e.package_id=s.package_id
 AND split_part(e.template_fqn,':',2)=s.module AND split_part(e.template_fqn,':',3)=s.entity
LEFT JOIN public.__transactions t ON t.ix=e.exercised_at_ix
WHERE e.exercised_at_offset>$1 AND e.witnesses && $3::text[]
ORDER BY e.exercised_at_offset,(e.exercise_event_id).node_id LIMIT 65;
