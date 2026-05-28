-- Per-space reserve price. Positive = normal reserve (minimum starting bid).
-- Negative = chore semantics (winner is paid to take on the obligation).
-- No CHECK constraint -- negatives are valid. Cannot reuse the AMOUNT
-- domain because that is constrained `>= 0`.
ALTER TABLE spaces
    ADD COLUMN reserve_price NUMERIC(20, 6) NOT NULL DEFAULT 0;

-- Allow user_values.value to be negative so chore valuations can be
-- expressed directly (e.g., "I'll accept down to -$5 for this chore").
ALTER TABLE user_values DROP CONSTRAINT user_values_value_check;

-- Collapse the five mode-specific treasury entry types into a single
-- `treasury_transfer` variant. The names asserted a *story* about each
-- transfer (debt settlement, credit purchase, allowance issuance, ...)
-- that often didn't match the actual motivation; the reason now lives
-- in the entry's note.
--
-- Postgres can't drop enum values in place. Rename the existing type,
-- define the new collapsed type, then ALTER COLUMN with a USING clause
-- that remaps the five old values to `treasury_transfer`. The CHECK
-- constraint on entry_type must be dropped first and recreated after,
-- since the literal it compares against (`auction_settlement`) would
-- otherwise try to resolve against the new type mid-migration.
ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_check;

ALTER TYPE ENTRY_TYPE RENAME TO ENTRY_TYPE_OLD;

CREATE TYPE ENTRY_TYPE AS ENUM (
    'transfer',
    'treasury_transfer',
    'auction_settlement',
    'balance_reset',
    'orphaned_account_transfer'
);

ALTER TABLE journal_entries
    ALTER COLUMN entry_type TYPE ENTRY_TYPE
    USING (
        CASE entry_type::TEXT
            WHEN 'issuance_grant_single'   THEN 'treasury_transfer'
            WHEN 'issuance_grant_bulk'     THEN 'treasury_transfer'
            WHEN 'credit_purchase'         THEN 'treasury_transfer'
            WHEN 'distribution_correction' THEN 'treasury_transfer'
            WHEN 'debt_settlement'         THEN 'treasury_transfer'
            ELSE entry_type::TEXT
        END
    )::ENTRY_TYPE;

DROP TYPE ENTRY_TYPE_OLD;

ALTER TABLE journal_entries
    ADD CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL);
