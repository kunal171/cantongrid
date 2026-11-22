-- Canonical payloads are immutable. Archival changes membership, not historical financial values.
-- Visibility is applied before both count and page; this live profile archives by prior-create policy.
WITH visible AS NOT MATERIALIZED (
    SELECT c.contract_id AS key
    FROM cantongrid.contracts c
    JOIN cantongrid.events created ON created.stream_key=c.stream_key AND created.event_id=c.create_event_id
    LEFT JOIN cantongrid.events archived ON archived.stream_key=c.stream_key AND archived.event_id=c.archive_event_id
    WHERE c.stream_key=$1 AND c.witnesses && $2::text[]
      AND (created.envelope#>>'{events,0,origin,offset}')::bigint <= $3
      AND (c.archive_event_id IS NULL OR (archived.envelope#>>'{events,0,origin,offset}')::bigint > $3)
), page AS MATERIALIZED (
    SELECT key FROM visible WHERE ($4::text IS NULL OR key > $4) ORDER BY key LIMIT $5
), encoded AS (
    SELECT page.key,jsonb_build_object('key',page.key,'value',jsonb_build_object(
        'contractId',c.contract_id,'template',c.template,'payload',c.payload,'createEventId',c.create_event_id)) value
    FROM page JOIN cantongrid.contracts c ON c.stream_key=$1 AND c.contract_id=page.key
)
SELECT (SELECT count(*) FROM visible),COALESCE((SELECT jsonb_agg(value ORDER BY key) FROM encoded),'[]'::jsonb);
