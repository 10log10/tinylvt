ALTER TABLE users ADD COLUMN balance NUMERIC(20, 6) NOT NULL DEFAULT 0;

-- Enum values can't be dropped; recreate the type without the value. The
-- USING cast fails loudly if any rounding_adjustment entries exist -- those
-- rows would have to be dealt with manually before downgrading.
ALTER TYPE entry_type RENAME TO entry_type_old;

CREATE TYPE entry_type AS ENUM (
    'transfer',
    'treasury_transfer',
    'auction_settlement',
    'balance_reset',
    'orphaned_account_transfer'
);

ALTER TABLE journal_entries
    ALTER COLUMN entry_type TYPE entry_type
    USING entry_type::text::entry_type;

DROP TYPE entry_type_old;
