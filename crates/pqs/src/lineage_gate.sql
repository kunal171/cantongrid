-- Unknown visibility/identity stops progress before any creation metadata is copied.
WITH selected AS (
 SELECT * FROM unnest($4::text[],$5::text[],$6::text[]) AS t(package_id,module,entity)
)
SELECT EXISTS (
 SELECT 1 FROM public.creates(NULL,$1,$2) c
 JOIN selected s ON c.creation_package_id=s.package_id
  AND split_part(c.template_fqn,':',2)=s.module AND split_part(c.template_fqn,':',3)=s.entity
 WHERE c.payload_type::text='template'
 AND (c.created_at_offset IS NULL OR (c.created_at_offset>$1
  AND (c.witnesses IS NULL OR cardinality(c.witnesses)=0
       OR (c.redaction_id IS NOT NULL AND c.witnesses && $3::text[]))))
);
