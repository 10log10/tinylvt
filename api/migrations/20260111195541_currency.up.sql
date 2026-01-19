-- Adds currency modes and a double-entry ledger to track transactions and
-- ensure payments are made.

-- # The currency modes
--
-- - points_allocation: members are issued points by the treasury to use in
--   auctions, and that go back to the treasury
-- - distributed_clearing: members issue IOUs to other members, which are
--   settled later
-- - deferred_payment: members issue IOUs to the treasury, settled later
-- - prepaid_credits: members buy credits from the treasury, which go back to
--   the treasury
--
-- mode                 | credit_limit | debts_callable | auction_settlement_flow | issuance_policy
-- ---------------------|--------------|----------------|-------------------------|----------------
-- points_allocation    |            0 |          false |             to_treasury |       allowance
-- distributed_clearing |  >=0 or null |            any |      equal_distribution |            none
-- deferred_payment     |  >=0 or null |            any |             to_treasury |            none
-- prepaid_credits      |            0 |            any |             to_treasury |        purchase
--
-- # Denomination
--
-- The currency is denominated in units defined by currency_name and
-- currency_symbol. Communities define what their currency represents (e.g.,
-- dollars, hours, bananas).
--
-- # Callability
--
-- For all but points_allocation, debts can optionally be callable. When debts
-- are callable, all debts carry a promise of settlement in the denominated
-- unit.
--
-- Without callable debts, the currency maintains its value either through the
-- cost to purchase it (prepaid_credits), or a finite credit limit
-- (distributed_clearing and deferred_payment).

CREATE TYPE CURRENCY_MODE AS ENUM (
    'points_allocation',
    'distributed_clearing',
    'deferred_payment',
    'prepaid_credits'
);

CREATE DOMAIN AMOUNT AS NUMERIC(20, 6) CHECK (VALUE >= 0);

ALTER TABLE communities
    ADD COLUMN currency_mode CURRENCY_MODE NOT NULL DEFAULT 'distributed_clearing',
    -- The default credit limit given to members within a community.
    -- Can be overridden on a per-member basis.
    -- Null means there is no limit.
    ADD COLUMN default_credit_limit AMOUNT,
    -- User-assigned name and symbol to currency
    ADD COLUMN currency_name VARCHAR(50) NOT NULL DEFAULT 'dollars',
    ADD COLUMN currency_symbol VARCHAR(4) NOT NULL DEFAULT '$',
    -- Whether debts can be called for settlement in the denominated unit
    ADD COLUMN debts_callable BOOLEAN NOT NULL DEFAULT true,
    -- Allowance settings for points_allocation
    ADD COLUMN allowance_amount AMOUNT,
    ADD COLUMN allowance_period INTERVAL,
    ADD COLUMN allowance_start TIMESTAMPTZ, -- Starting point for automated issuance
    -- Points allocation constraints
    ADD CHECK (currency_mode != 'points_allocation' OR (
        debts_callable = false
        AND default_credit_limit IS NOT NULL AND default_credit_limit = 0
        AND allowance_amount IS NOT NULL
        AND allowance_period IS NOT NULL
        AND allowance_start IS NOT NULL
    )),
    -- Distributed clearing and deferred payment constraints
    ADD CHECK (currency_mode NOT IN ('distributed_clearing', 'deferred_payment') OR (
        allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
        -- Without callable debts, must have a finite credit limit to prevent
        -- infinite debt accumulation
        AND (debts_callable = true OR default_credit_limit IS NOT NULL)
    )),
    -- Prepaid credits constraints
    ADD CHECK (currency_mode != 'prepaid_credits' OR (
        default_credit_limit IS NOT NULL AND default_credit_limit = 0
        AND allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
    ));

ALTER TABLE community_members
    ADD COLUMN credit_limit AMOUNT;  -- Credit limit override for this member
    -- TODO: add a balance_cached column with a materialized calculation of the user's balance?

-- Currency account types:
-- - 'member_main': a member's personal account
-- - 'community_treasury': the central community account that issues currency
--   or sinks payments when the mode is not distributed_clearing
CREATE TYPE ACCOUNT_OWNER_TYPE AS ENUM (
    'member_main',
    'community_treasury'
);

CREATE TABLE accounts (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id  UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    owner_type    ACCOUNT_OWNER_TYPE NOT NULL,
    owner_id      UUID REFERENCES users (id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL,
    -- For member_main accounts, owner_id must be set
    -- For community_treasury accounts, owner_id must be null
    CHECK ((owner_type = 'member_main' AND owner_id IS NOT NULL) OR
         (owner_type = 'community_treasury' AND owner_id IS NULL))
);

CREATE TYPE ENTRY_TYPE AS ENUM ('issuance_grant', 'auction_settlement', 'transfer');

-- entries in the ledger
-- each entry has legs in journal_lines, which have amounts that sum to 0
CREATE TABLE journal_entries (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id      UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    entry_type        ENTRY_TYPE NOT NULL,
    idempotency_key   UUID NOT NULL,
    auction_id        UUID REFERENCES auctions (id), -- optional auction ref
    created_at        TIMESTAMPTZ NOT NULL,
    UNIQUE (idempotency_key),
    CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL)
); -- no metadata JSONB; when new metadata forms are needed, add those as cols

CREATE TABLE journal_lines (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id          UUID NOT NULL REFERENCES journal_entries (id) ON DELETE CASCADE,
    account_id        UUID NOT NULL REFERENCES accounts (id),
    amount            NUMERIC(20, 6) NOT NULL
);

-- Application is responsible for ensuring that the sum of journal lines for an
-- entry_id is 0

-- Create treasury accounts for all existing communities
INSERT INTO accounts (community_id, owner_type, owner_id, created_at)
SELECT id, 'community_treasury', NULL, NOW()  -- NOW() OK in backfill
FROM communities;
