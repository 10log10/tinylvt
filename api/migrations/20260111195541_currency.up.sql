-- Adds currency modes and a double-entry ledger to track transactions and
-- ensure payments are made.

-- The currency modes
--
-- - points_allocation: members are issued points by the treasury to use in
--   auctions, and that go back to the treasury
-- - distributed_clearing: members issue IOUs to other members, which are
--   settled later
-- - deferred_payment: members issue IOUs to the treasury, settled later
-- - prepaid_credits: members buy credits from the treasury, which go back to
--   the treasury
--
-- mode                 | credit_limit | tether_type | auction_settlement_flow | issuance_policy
-- ---------------------|--------------|-------------|-------------------------|----------------
-- points_allocation    |            0 |        none |             to_treasury |       allowance
-- distributed_clearing |  >=0 or null | unit_of_account |  equal_distribution |            none
-- deferred_payment     |  >=0 or null | unit_of_account |         to_treasury |            none 
-- prepaid_credits      |            0 | UOA or redeemable |       to_treasury |        purchase

CREATE TYPE CURRENCY_MODE AS ENUM (
    'points_allocation',
    'distributed_clearing',
    'deferred_payment',
    'prepaid_credits'
);

CREATE TYPE TETHER_TYPE AS ENUM (
    'none',
    'unit_of_account',
    'redeemable'
);

CREATE DOMAIN AMOUNT AS NUMERIC(20, 6) CHECK (VALUE >= 0);

ALTER TABLE communities
    ADD COLUMN currency_mode CURRENCY_MODE NOT NULL DEFAULT 'distributed_clearing',
    -- The default credit limit given to members within a community.
    -- Can be overridden on a per-member basis.
    -- Null means there is no limit.
    ADD COLUMN default_credit_limit AMOUNT,
    -- User-assigned name and symbol to currency.
    -- If tethered, can default to that of the tether currency.
    ADD COLUMN currency_name VARCHAR(50) NOT NULL DEFAULT 'dollars',
    ADD COLUMN currency_symbol VARCHAR(4) NOT NULL DEFAULT '$',
    ADD COLUMN tether_type TETHER_TYPE NOT NULL DEFAULT 'unit_of_account',
    -- ISO 4217 currency code
    ADD COLUMN tether_currency CHAR(3) CHECK (tether_currency ~ '^[A-Z]{3}$') DEFAULT 'USD',
    -- Allowance settings for points_allocation
    ADD COLUMN allowance_amount AMOUNT,
    ADD COLUMN allowance_period INTERVAL,
    ADD COLUMN allowance_start TIMESTAMPTZ, -- Starting point for automated issuance
    -- Tether constraints
    ADD CHECK ((tether_type = 'none') = (tether_currency IS NULL)),
    -- Points allocation constraints
    ADD CHECK (currency_mode != 'points_allocation' OR (
        tether_type = 'none'
        AND default_credit_limit IS NOT NULL AND default_credit_limit = 0
        AND allowance_amount IS NOT NULL
        AND allowance_period IS NOT NULL
        AND allowance_start IS NOT NULL
    )),
    -- Distributed clearing and deferred payment constraints
    ADD CHECK (currency_mode NOT IN ('distributed_clearing', 'deferred_payment') OR (
        tether_type = 'unit_of_account'
        AND allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
    )),
    -- Prepaid credits constraints
    ADD CHECK (currency_mode != 'prepaid_credits' OR (
        tether_type IN ('unit_of_account', 'redeemable')
        AND default_credit_limit IS NOT NULL AND default_credit_limit = 0
        AND allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
    ));

ALTER TABLE community_members
    ADD COLUMN credit_limit AMOUNT;  -- Credit limit override for this member
