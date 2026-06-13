ALTER TABLE auctions DROP COLUMN was_canceled;

-- start_at must be NOT NULL again; backfill unscheduled auctions with their
-- creation time so the constraint can be restored.
UPDATE auctions SET start_at = created_at WHERE start_at IS NULL;
ALTER TABLE auctions ALTER COLUMN start_at SET NOT NULL;
