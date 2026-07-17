ALTER TABLE auction_rounds
    ADD COLUMN proxy_bidding_last_processed_at TIMESTAMPTZ,
    ADD COLUMN proxy_bidding_failure_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN proxy_bidding_last_failed_at TIMESTAMPTZ;

ALTER TABLE use_proxy_bidding
    DROP COLUMN needs_processing;

DROP TABLE proxy_round_processing;
