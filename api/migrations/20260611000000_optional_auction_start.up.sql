-- Allow auctions to be created without a scheduled start time. A NULL
-- start_at means the auction is waiting to be started manually (or
-- scheduled later) by a coleader+; the scheduler ignores such auctions.
ALTER TABLE auctions ALTER COLUMN start_at DROP NOT NULL;

-- Soft-delete-style cancellation. A canceled auction has end_at set (which
-- stops the scheduler from processing it further, including settlement) and
-- was_canceled = TRUE so the UI can distinguish cancellation from a normal
-- conclusion. Canceled auctions never get a settlement journal entry, so
-- they remain hard-deletable (journal_entries.auction_id is ON DELETE
-- RESTRICT for settled auctions).
ALTER TABLE auctions ADD COLUMN was_canceled BOOLEAN NOT NULL DEFAULT FALSE;
