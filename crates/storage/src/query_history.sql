WITH visible AS MATERIALIZED (
    SELECT event_id AS key,
           jsonb_build_object('eventId',event_id,'contractId',contract_id,'position',position,
                              'kind',kind,'envelope',envelope) AS value
    FROM cantongrid.events
    WHERE stream_key=$1 AND CASE WHEN jsonb_typeof(visibility_clauses)='array' THEN
        jsonb_array_length(visibility_clauses)>0 AND NOT EXISTS (
            SELECT 1 FROM jsonb_array_elements(visibility_clauses) clause
            WHERE CASE WHEN jsonb_typeof(clause)='array' THEN NOT EXISTS (
                SELECT 1 FROM jsonb_array_elements(clause) party
                WHERE jsonb_typeof(party)='string' AND (party #>> '{}')=ANY($2::text[])
            ) ELSE true END
        ) ELSE false END
), page AS (
    SELECT key,value FROM visible WHERE ($3::text IS NULL OR key > $3)
    ORDER BY key LIMIT $4
)
SELECT (SELECT count(*) FROM visible),
       COALESCE((SELECT jsonb_agg(jsonb_build_object('key',key,'value',value) ORDER BY key) FROM page),'[]'::jsonb);
