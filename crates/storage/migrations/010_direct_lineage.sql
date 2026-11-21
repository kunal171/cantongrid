-- Owned destination only. Provenance never grants access to an endpoint contract.
CREATE TABLE cantongrid.lineage_pages (
 stream_key text NOT NULL,
 revision bigint NOT NULL,
 envelope jsonb NOT NULL CHECK(jsonb_typeof(envelope)='array'),
 PRIMARY KEY(stream_key,revision),
 FOREIGN KEY(stream_key,revision) REFERENCES cantongrid.batches
);
CREATE TABLE cantongrid.creation_events (
 stream_key text NOT NULL,
 source_offset bigint NOT NULL CHECK(source_offset>0),
 node_id integer NOT NULL CHECK(node_id>=0),
 revision bigint NOT NULL,
 envelope jsonb NOT NULL CHECK(envelope->>'version'='1'),
 PRIMARY KEY(stream_key,source_offset,node_id),
 FOREIGN KEY(stream_key,revision) REFERENCES cantongrid.lineage_pages
);
CREATE TABLE cantongrid.lineage_edges (
 stream_key text NOT NULL,
 edge_key text NOT NULL,
 revision bigint NOT NULL,
 parent_contract text NOT NULL,
 child_contract text NOT NULL,
 envelope jsonb NOT NULL CHECK(envelope->>'version'='1'),
 visibility_clauses jsonb NOT NULL CHECK(jsonb_typeof(visibility_clauses)='array' AND jsonb_array_length(visibility_clauses)>0),
 PRIMARY KEY(stream_key,edge_key),
 FOREIGN KEY(stream_key,revision) REFERENCES cantongrid.lineage_pages
);
CREATE INDEX lineage_parent ON cantongrid.lineage_edges(stream_key,parent_contract,edge_key);
CREATE INDEX lineage_child ON cantongrid.lineage_edges(stream_key,child_contract,edge_key);
CREATE TABLE cantongrid.lineage_outbox (
 delivery_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 stream_key text NOT NULL,
 edge_key text NOT NULL,
 UNIQUE(stream_key,edge_key),
 FOREIGN KEY(stream_key,edge_key) REFERENCES cantongrid.lineage_edges
);
REVOKE ALL ON cantongrid.lineage_pages,cantongrid.creation_events,cantongrid.lineage_edges,cantongrid.lineage_outbox FROM PUBLIC;
REVOKE ALL ON SEQUENCE cantongrid.lineage_outbox_delivery_id_seq FROM PUBLIC;
