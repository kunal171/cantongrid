-- Private consumer progress over the existing transactional lifecycle outbox.
-- UUID generation is built into supported PostgreSQL; no extension is installed.
CREATE TABLE cantongrid.delivery_subscriptions (
    id text PRIMARY KEY DEFAULT gen_random_uuid()::text,
    stream_key text NOT NULL REFERENCES cantongrid.streams(stream_key) ON DELETE CASCADE,
    principal_fingerprint text NOT NULL CHECK (principal_fingerprint ~ '^[0-9a-f]{64}$'),
    authorization_binding text NOT NULL CHECK (authorization_binding ~ '^[0-9a-f]{64}$'),
    client_name text NOT NULL CHECK (client_name ~ '^[a-zA-Z][a-zA-Z0-9_-]{0,63}$'),
    start_mode text NOT NULL CHECK (start_mode IN ('now','retained')),
    parties text[] NOT NULL CHECK (cardinality(parties)>0),
    acknowledged bigint NOT NULL CHECK (acknowledged>=0),
    lost_through bigint NOT NULL DEFAULT 0 CHECK (lost_through>=0),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT now()+interval '24 hours',
    UNIQUE(stream_key,principal_fingerprint,authorization_binding,client_name)
);
CREATE TABLE cantongrid.delivery_receipts (
    id text PRIMARY KEY DEFAULT gen_random_uuid()::text,
    subscription_id text NOT NULL REFERENCES cantongrid.delivery_subscriptions(id) ON DELETE CASCADE,
    after_delivery bigint NOT NULL CHECK (after_delivery>=0),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX delivery_receipts_subscription ON cantongrid.delivery_receipts(subscription_id,after_delivery);
CREATE INDEX delivery_subscriptions_expiry ON cantongrid.delivery_subscriptions(stream_key,expires_at);
-- Stream-specific pruning never needs a public/global sequence cursor.
CREATE INDEX outbox_stream_delivery ON cantongrid.outbox(stream_key,delivery_id);
REVOKE ALL ON cantongrid.delivery_subscriptions,cantongrid.delivery_receipts FROM PUBLIC;
