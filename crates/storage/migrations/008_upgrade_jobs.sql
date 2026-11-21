-- Private bounded snapshot staging; publish/cancel phases temporarily gate this stream's reads/writes.
CREATE TABLE cantongrid.selection_upgrade_jobs (
    stream_key text NOT NULL REFERENCES cantongrid.streams(stream_key),
    fingerprint text NOT NULL,
    manifest jsonb NOT NULL,
    phase text NOT NULL CHECK (phase IN ('snapshot','ready','publishing','complete','cancelling','cancelled')),
    source_cursor text NOT NULL,
    pages bigint NOT NULL DEFAULT 0 CHECK (pages >= 0),
    applied bigint NOT NULL DEFAULT 0 CHECK (applied >= 0),
    contracts bigint NOT NULL DEFAULT 0 CHECK (contracts >= 0),
    staged_bytes bigint NOT NULL DEFAULT 0 CHECK (staged_bytes >= 0),
    cleanup_after bigint NOT NULL DEFAULT 0 CHECK (cleanup_after >= 0),
    PRIMARY KEY (stream_key,fingerprint)
);
CREATE UNIQUE INDEX selection_upgrade_one_active ON cantongrid.selection_upgrade_jobs(stream_key)
    WHERE phase NOT IN ('complete','cancelled');
CREATE TABLE cantongrid.selection_upgrade_pages (
    stream_key text NOT NULL,
    fingerprint text NOT NULL,
    page bigint NOT NULL CHECK (page > 0),
    envelope jsonb NOT NULL,
    PRIMARY KEY (stream_key,fingerprint,page),
    FOREIGN KEY (stream_key,fingerprint) REFERENCES cantongrid.selection_upgrade_jobs(stream_key,fingerprint)
);
REVOKE ALL ON cantongrid.selection_upgrade_jobs,cantongrid.selection_upgrade_pages FROM PUBLIC;
