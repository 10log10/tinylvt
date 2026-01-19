-- Reverses the currency migration by removing all currency-related columns and types

ALTER TABLE community_members
    DROP COLUMN credit_limit;

ALTER TABLE communities
    DROP COLUMN currency_mode,
    DROP COLUMN default_credit_limit,
    DROP COLUMN currency_name,
    DROP COLUMN currency_symbol,
    DROP COLUMN tether_type,
    DROP COLUMN tether_currency,
    DROP COLUMN allowance_amount,
    DROP COLUMN allowance_period,
    DROP COLUMN allowance_start;

DROP DOMAIN amount;

DROP TYPE TETHER_TYPE;

DROP TYPE CURRENCY_MODE;
