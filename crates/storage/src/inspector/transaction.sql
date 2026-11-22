-- Source metadata alone never grants contract access. Remove hidden endpoints and source witnesses
-- BEFORE assigning ordinals. Raw node numbers, descendants, arguments/results and witnesses stay private.
WITH nodes AS MATERIALIZED (
 SELECT n.node_id,0 AS kind_order,jsonb_build_object(
    'kind','create','contractId',c.contract_id,'template',c.template) AS value
 FROM cantongrid.creation_events n
 JOIN cantongrid.contracts c ON c.stream_key=n.stream_key AND c.contract_id=n.envelope->>'contract'
 WHERE n.stream_key=$1 AND n.envelope->>'transaction_id'=$3 AND c.witnesses && $2::text[]
 AND CASE WHEN jsonb_typeof(n.envelope->'witnesses')='array' THEN EXISTS(
   SELECT 1 FROM jsonb_array_elements(n.envelope->'witnesses') p
   WHERE jsonb_typeof(p)='string' AND p #>> '{}' = ANY($2::text[])
 ) ELSE false END
 UNION ALL
 SELECT n.node_id,1,jsonb_build_object('kind','exercise','contractId',c.contract_id,
    'template',c.template,'choice',n.envelope->>'choice','consuming',n.envelope->'consuming')
 FROM cantongrid.exercise_events n
 JOIN cantongrid.contracts c ON c.stream_key=n.stream_key AND c.contract_id=n.envelope->>'contract'
 WHERE n.stream_key=$1 AND n.envelope->>'transaction_id'=$3 AND c.witnesses && $2::text[]
 AND CASE WHEN jsonb_typeof(n.envelope->'witnesses')='array' THEN EXISTS(
   SELECT 1 FROM jsonb_array_elements(n.envelope->'witnesses') p
   WHERE jsonb_typeof(p)='string' AND p #>> '{}' = ANY($2::text[])
 ) ELSE false END
 UNION ALL
 SELECT n.node_id,2,jsonb_build_object('kind','archive','contractId',c.contract_id,'template',c.template)
 FROM cantongrid.exercise_events n
 JOIN cantongrid.contracts c ON c.stream_key=n.stream_key AND c.contract_id=n.envelope->>'contract'
 JOIN cantongrid.events e ON e.stream_key=n.stream_key AND e.contract_id=c.contract_id AND e.kind='archive'
 WHERE n.stream_key=$1 AND n.envelope->>'transaction_id'=$3 AND n.envelope->'consuming'='true'::jsonb
 AND e.envelope->'events'->0->'origin'->>'offset'=n.source_offset::text
 AND c.witnesses && $2::text[]
 AND CASE WHEN jsonb_typeof(n.envelope->'witnesses')='array' THEN EXISTS(
   SELECT 1 FROM jsonb_array_elements(n.envelope->'witnesses') p
   WHERE jsonb_typeof(p)='string' AND p #>> '{}' = ANY($2::text[])
 ) ELSE false END
 AND CASE WHEN jsonb_typeof(e.visibility_clauses)='array' THEN jsonb_array_length(e.visibility_clauses)>0
 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(e.visibility_clauses) clause WHERE
   CASE WHEN jsonb_typeof(clause)='array' THEN NOT EXISTS(
     SELECT 1 FROM jsonb_array_elements(clause) p WHERE jsonb_typeof(p)='string' AND p #>> '{}' = ANY($2::text[])
   ) ELSE true END
 ) ELSE false END
), visible AS MATERIALIZED (
 SELECT row_number() OVER(ORDER BY node_id,kind_order) AS ordinal,value FROM nodes
), page AS MATERIALIZED (
 SELECT ordinal,value FROM visible WHERE ordinal>$4 ORDER BY ordinal LIMIT $5
)
SELECT (SELECT count(*) FROM visible),coalesce((SELECT jsonb_agg(
 jsonb_build_object('key',ordinal::text,'value',value||jsonb_build_object('ordinal',ordinal::text)) ORDER BY ordinal
) FROM page),'[]'::jsonb);
