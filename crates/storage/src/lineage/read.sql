-- Hide unauthorized edges before totals, ordering and pagination. Endpoint policies are independent.
WITH visible AS NOT MATERIALIZED (
 SELECT e.* FROM cantongrid.lineage_edges e
 JOIN cantongrid.contracts p ON p.stream_key=e.stream_key AND p.contract_id=e.parent_contract
 JOIN cantongrid.contracts c ON c.stream_key=e.stream_key AND c.contract_id=e.child_contract
 WHERE e.stream_key=$1
  AND (($4::boolean AND e.parent_contract=$3) OR (NOT $4::boolean AND e.child_contract=$3))
  AND p.witnesses && $2::text[] AND c.witnesses && $2::text[]
  AND CASE WHEN jsonb_typeof(e.visibility_clauses)='array' THEN
   jsonb_array_length(e.visibility_clauses)>0 AND NOT EXISTS (
    SELECT 1 FROM jsonb_array_elements(e.visibility_clauses) clause
    WHERE CASE WHEN jsonb_typeof(clause)='array' THEN NOT EXISTS (
     SELECT 1 FROM jsonb_array_elements(clause) party
     WHERE jsonb_typeof(party)='string' AND (party #>> '{}')=ANY($2::text[])
    ) ELSE true END
   ) ELSE false END
), page AS MATERIALIZED (
 SELECT * FROM visible WHERE ($5::text IS NULL OR edge_key>$5) ORDER BY edge_key LIMIT $6
)
SELECT (SELECT count(*) FROM visible),coalesce((
 SELECT jsonb_agg(jsonb_build_object('key',edge_key,'value',jsonb_build_object(
   'parentContractId',parent_contract,'childContractId',child_contract,
   'relationship','observed_direct','proof','complete_source_subtree',
   'sourceOffset',envelope->>'offset','exerciseNodeId',envelope->>'parent_node',
   'creationNodeId',envelope->>'child_node','choice',envelope->>'choice',
   'parentTemplate',envelope->'parent_template','childTemplate',envelope->'child_template'
 )) ORDER BY edge_key) FROM page
),'[]'::jsonb);
