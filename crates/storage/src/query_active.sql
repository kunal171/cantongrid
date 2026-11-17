WITH visible AS MATERIALIZED (
    SELECT contract_id AS key,
           jsonb_build_object('contractId',contract_id,'template',template,'payload',payload,
                              'createEventId',create_event_id) AS value
    FROM cantongrid.contracts
    WHERE stream_key=$1 AND active AND witnesses && $2::text[]
), page AS (
    SELECT key,value FROM visible WHERE ($3::text IS NULL OR key > $3)
    ORDER BY key LIMIT $4
)
SELECT (SELECT count(*) FROM visible),
       COALESCE((SELECT jsonb_agg(jsonb_build_object('key',key,'value',value) ORDER BY key) FROM page),'[]'::jsonb);
