-- Apply every dependency clause before selecting even the extra has-more row.
SELECT o.delivery_id,e.event_id,e.contract_id,e.kind,e.envelope
FROM cantongrid.outbox o JOIN cantongrid.events e USING(stream_key,event_id)
WHERE o.stream_key=$1 AND o.delivery_id>$2
AND CASE WHEN jsonb_typeof(e.visibility_clauses)='array' THEN
    jsonb_array_length(e.visibility_clauses)>0 AND NOT EXISTS(
        SELECT 1 FROM jsonb_array_elements(e.visibility_clauses) clause
        WHERE CASE WHEN jsonb_typeof(clause)='array' THEN NOT EXISTS(
            SELECT 1 FROM jsonb_array_elements(clause) party
            WHERE jsonb_typeof(party)='string' AND (party #>> '{}')=ANY($3::text[])
        ) ELSE true END
    ) ELSE false END
ORDER BY o.delivery_id LIMIT $4;
