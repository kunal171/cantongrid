-- A private fixed baseline paired with its original outbox anchor; neither ordinal is public.
ALTER TABLE cantongrid.delivery_subscriptions
    ADD COLUMN baseline_offset bigint,
    ADD COLUMN baseline_after bigint,
    ADD CONSTRAINT delivery_bootstrap_pair CHECK (
        (baseline_offset IS NULL AND baseline_after IS NULL)
        OR (baseline_offset IS NOT NULL AND baseline_after IS NOT NULL
            AND baseline_offset >= 0 AND baseline_after >= 0
            AND start_mode = 'now' AND acknowledged >= baseline_after)
    );
