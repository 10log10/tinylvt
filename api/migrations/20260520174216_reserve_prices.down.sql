-- Restore the pre-collapse ENTRY_TYPE enum. All previously-collapsed
-- rows land on `debt_settlement` since we cannot recover the original
-- semantic split from the data alone (the five old values all map to
-- `treasury_transfer` going forward, so the inverse loses information).
-- Pick `debt_settlement` as the rollback target -- it is the most
-- generic of the five and least likely to mislead in a transient
-- down-migration scenario.
ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_check;

ALTER TYPE ENTRY_TYPE RENAME TO ENTRY_TYPE_NEW;

CREATE TYPE ENTRY_TYPE AS ENUM (
    'issuance_grant_single',
    'issuance_grant_bulk',
    'credit_purchase',
    'distribution_correction',
    'debt_settlement',
    'auction_settlement',
    'transfer',
    'balance_reset',
    'orphaned_account_transfer'
);

ALTER TABLE journal_entries
    ALTER COLUMN entry_type TYPE ENTRY_TYPE
    USING (
        CASE entry_type::TEXT
            WHEN 'treasury_transfer' THEN 'debt_settlement'
            ELSE entry_type::TEXT
        END
    )::ENTRY_TYPE;

DROP TYPE ENTRY_TYPE_NEW;

ALTER TABLE journal_entries
    ADD CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL);

ALTER TABLE user_values ADD CONSTRAINT user_values_value_check CHECK (value >= 0);
ALTER TABLE spaces DROP COLUMN reserve_price;
