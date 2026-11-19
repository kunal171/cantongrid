-- Count and page authorized identities before loading the bounded event envelopes.
WITH visible AS NOT MATERIALIZED (
    SELECT event_id AS key
    FROM cantongrid.events
    WHERE stream_key=$1 AND CASE WHEN jsonb_typeof(visibility_clauses)='array' THEN
        jsonb_array_length(visibility_clauses)>0 AND NOT EXISTS (
            SELECT 1 FROM jsonb_array_elements(visibility_clauses) clause
            WHERE CASE WHEN jsonb_typeof(clause)='array' THEN NOT EXISTS (
                SELECT 1 FROM jsonb_array_elements(clause) party
                WHERE jsonb_typeof(party)='string' AND (party #>> '{}')=ANY($2::text[])
            ) ELSE true END
        ) ELSE false END
), page AS MATERIALIZED (
    SELECT key FROM visible WHERE ($3::text IS NULL OR key > $3)
    ORDER BY key LIMIT $4
), encoded AS (
    SELECT page.key,
           jsonb_build_object('eventId',event_id,'contractId',contract_id,'position',position,
                              'kind',kind,'envelope',envelope) AS value
    FROM page JOIN cantongrid.events e ON e.stream_key=$1 AND e.event_id=page.key
)
SELECT (SELECT count(*) FROM visible),
       COALESCE((SELECT jsonb_agg(jsonb_build_object('key',key,'value',value) ORDER BY key) FROM encoded),'[]'::jsonb);
