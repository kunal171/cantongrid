-- Owned destination only. Exercise evidence never mutates contracts or the lifecycle outbox.
CREATE TABLE cantongrid.exercise_pages (
    stream_key text NOT NULL,
    revision bigint NOT NULL,
    envelope jsonb NOT NULL CHECK (jsonb_typeof(envelope) = 'array'),
    PRIMARY KEY (stream_key, revision),
    FOREIGN KEY (stream_key, revision) REFERENCES cantongrid.batches
);
CREATE TABLE cantongrid.exercise_events (
    stream_key text NOT NULL,
    source_offset bigint NOT NULL CHECK (source_offset > 0),
    node_id integer NOT NULL CHECK (node_id >= 0),
    revision bigint NOT NULL,
    envelope jsonb NOT NULL CHECK (envelope->>'version' = '1'),
    PRIMARY KEY (stream_key, source_offset, node_id),
    FOREIGN KEY (stream_key, revision) REFERENCES cantongrid.exercise_pages
);
CREATE TABLE cantongrid.exercise_outbox (
    delivery_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    stream_key text NOT NULL,
    source_offset bigint NOT NULL,
    node_id integer NOT NULL,
    UNIQUE (stream_key, source_offset, node_id),
    FOREIGN KEY (stream_key, source_offset, node_id) REFERENCES cantongrid.exercise_events
);
REVOKE ALL ON cantongrid.exercise_pages,cantongrid.exercise_events,cantongrid.exercise_outbox FROM PUBLIC;
REVOKE ALL ON SEQUENCE cantongrid.exercise_outbox_delivery_id_seq FROM PUBLIC;
