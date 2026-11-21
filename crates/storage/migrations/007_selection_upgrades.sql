-- Reviewed additive selection handoffs; source checkpoints and historical batches remain intact.
CREATE TABLE cantongrid.selection_upgrades (
    stream_key text NOT NULL REFERENCES cantongrid.streams(stream_key),
    fingerprint text NOT NULL,
    manifest jsonb NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (stream_key, fingerprint)
);
REVOKE ALL ON cantongrid.selection_upgrades FROM PUBLIC;
