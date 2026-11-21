-- Unknown visibility must stop progress without retrieving private arguments or results.
WITH selected AS (
 SELECT * FROM unnest($4::text[],$5::text[],$6::text[]) AS t(package_id,module,entity)
)
SELECT EXISTS (
 SELECT 1 FROM public.exercises(NULL,$1,$2) e
 JOIN selected s ON e.package_id=s.package_id
  AND split_part(e.template_fqn,':',2)=s.module AND split_part(e.template_fqn,':',3)=s.entity
 WHERE e.exercised_at_offset IS NULL
  OR (e.exercised_at_offset>$1
      AND (e.witnesses IS NULL OR cardinality(e.witnesses)=0
           OR (e.redaction_id IS NOT NULL AND e.witnesses && $3::text[])))
);
