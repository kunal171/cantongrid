-- Keep count/page predicates indexable without spooling all visible rows. Encode only the page.
WITH visible AS NOT MATERIALIZED (
    SELECT contract_id AS key
    FROM cantongrid.contracts
    WHERE stream_key=$1 AND active AND witnesses && $2::text[]
), page AS MATERIALIZED (
    SELECT key FROM visible WHERE ($3::text IS NULL OR key > $3)
    ORDER BY key LIMIT $4
), encoded AS (
    SELECT page.key,
           jsonb_build_object('contractId',contract_id,'template',template,'payload',payload,
                              'createEventId',create_event_id) AS value
    FROM page JOIN cantongrid.contracts c ON c.stream_key=$1 AND c.contract_id=page.key
)
SELECT (SELECT count(*) FROM visible),
       COALESCE((SELECT jsonb_agg(jsonb_build_object('key',key,'value',value) ORDER BY key) FROM encoded),'[]'::jsonb);
