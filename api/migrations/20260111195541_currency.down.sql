-- Reverses the currency migration by removing all currency-related columns,
-- tables, and types

-- Drop tables in dependency order
DROP TABLE journal_lines;

DROP TABLE journal_entries;

DROP TABLE accounts;

-- Drop columns from communities table
ALTER TABLE communities
    DROP COLUMN currency_mode,
    DROP COLUMN default_credit_limit,
    DROP COLUMN currency_name,
    DROP COLUMN currency_symbol,
    DROP COLUMN currency_minor_units,
    DROP COLUMN debts_callable,
    DROP COLUMN balances_visible_to_members,
    DROP COLUMN allowance_amount,
    DROP COLUMN allowance_period,
    DROP COLUMN allowance_start;

-- Drop types in reverse dependency order
DROP TYPE ENTRY_TYPE;

DROP TYPE ACCOUNT_OWNER_TYPE;

DROP TYPE CURRENCY_MODE;

DROP DOMAIN AMOUNT;
