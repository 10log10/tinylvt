-- Granular per-(round, user) proxy bidding processing. Replaces the
-- round-level watermark/failure columns on auction_rounds: one user's
-- failure no longer puts the whole round into backoff, one user's
-- settings change no longer reprocesses everyone, and re-selection is
-- driven by a writer-set dirty flag instead of a timestamp watermark
-- (which lost writes that straddled the processor's read).

-- Per-(round, user) processing marker. An explicit marker row is needed
-- because "processed, but no surplus so zero bids" is indistinguishable
-- from "unprocessed" via bids alone. processed_at is informational;
-- re-selection is driven by marker existence (per-round baseline), the
-- needs_processing flag (mid-round change), and failure backoff. A
-- marker can exist with processed_at NULL when the first attempt fails.
CREATE TABLE proxy_round_processing (
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    processed_at TIMESTAMPTZ,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_failed_at TIMESTAMPTZ,
    PRIMARY KEY (round_id, user_id)
);

-- Writer-side dirty flag: set TRUE in the writer's own transaction by
-- proxy settings and user-value saves; cleared only by the processor's
-- claim transaction. The flag derives re-selection ordering from the
-- database's own serialization instead of clock comparisons.
ALTER TABLE use_proxy_bidding
    ADD COLUMN needs_processing BOOLEAN NOT NULL DEFAULT TRUE;

ALTER TABLE auction_rounds
    DROP COLUMN proxy_bidding_last_processed_at,
    DROP COLUMN proxy_bidding_failure_count,
    DROP COLUMN proxy_bidding_last_failed_at;
