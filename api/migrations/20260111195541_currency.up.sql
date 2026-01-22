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
-- Mode Configuration:
--
-- mode                 | credit_limit | debts_callable
-- ---------------------|--------------|---------------
-- points_allocation    |            0 |          false
-- distributed_clearing |  >=0 or null |            any
-- deferred_payment     |  >=0 or null |            any
-- prepaid_credits      |            0 |            any
--
-- Mode Behavior:
--
-- mode                 | auction_settlement  | issuance_policy
-- ---------------------|---------------------|----------------
-- points_allocation    | to_treasury         | allowance
-- distributed_clearing | equal_distribution  | none
-- deferred_payment     | to_treasury         | none
-- prepaid_credits      | to_treasury         | purchase
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
    ADD COLUMN currency_mode CURRENCY_MODE NOT NULL
        DEFAULT 'distributed_clearing',
    -- The default credit limit given to members within a community.
    -- Can be overridden on a per-member basis.
    -- Null means there is no limit.
    ADD COLUMN default_credit_limit AMOUNT,
    -- User-assigned name and symbol to currency
    ADD COLUMN currency_name VARCHAR(50) NOT NULL DEFAULT 'dollars',
    ADD COLUMN currency_symbol VARCHAR(4) NOT NULL DEFAULT '$',
    -- Whether debts can be called for settlement in denominated unit
    ADD COLUMN debts_callable BOOLEAN NOT NULL DEFAULT true,
    -- Whether ordinary members can see all member balances/limits
    -- Coleaders/leaders always see all; this affects member visibility
    ADD COLUMN balances_visible_to_members BOOLEAN NOT NULL
        DEFAULT true,
    -- Allowance settings for points_allocation
    ADD COLUMN allowance_amount AMOUNT,
    ADD COLUMN allowance_period INTERVAL,
    -- Starting point for automated issuance
    ADD COLUMN allowance_start TIMESTAMPTZ,
    -- Points allocation constraints
    ADD CHECK (currency_mode != 'points_allocation' OR (
        debts_callable = false
        AND default_credit_limit IS NOT NULL
        AND default_credit_limit = 0
        AND allowance_amount IS NOT NULL
        AND allowance_period IS NOT NULL
        AND allowance_start IS NOT NULL
    )),
    -- Distributed clearing and deferred payment constraints
    ADD CHECK (
        currency_mode NOT IN ('distributed_clearing', 'deferred_payment')
        OR (
            allowance_amount IS NULL
            AND allowance_period IS NULL
            AND allowance_start IS NULL
            -- Without callable debts, must have finite credit limit
            -- to prevent infinite debt accumulation
            AND (debts_callable = true OR default_credit_limit IS NOT NULL)
        )
    ),
    -- Prepaid credits constraints
    ADD CHECK (currency_mode != 'prepaid_credits' OR (
        default_credit_limit IS NOT NULL
        AND default_credit_limit = 0
        AND allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
    ));

-- Currency account types:
-- - 'member_main': a member's personal account
-- - 'community_treasury': the central community account that issues
--   currency or sinks payments when mode is not distributed_clearing
CREATE TYPE ACCOUNT_OWNER_TYPE AS ENUM (
    'member_main',
    'community_treasury'
);

CREATE TABLE accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id    UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    owner_type      ACCOUNT_OWNER_TYPE NOT NULL,
    owner_id        UUID REFERENCES users (id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL,
    -- Materialized balance kept in sync by application
    -- Positive = credit balance, Negative = debt
    balance_cached  NUMERIC(20, 6) NOT NULL DEFAULT 0,
    -- Credit limit for this account
    -- NULL = use community default_credit_limit
    -- Only applies to member_main accounts; treasury has no limit
    credit_limit    AMOUNT,
    -- Application enforces: balance_cached >=
    --   -COALESCE(credit_limit, community.default_credit_limit, infinity)
    -- For member_main accounts, owner_id must be set
    -- For community_treasury accounts, owner_id must be null
    CHECK ((owner_type = 'member_main' AND owner_id IS NOT NULL) OR
           (owner_type = 'community_treasury' AND owner_id IS NULL))
);

-- Journal entry types
--
-- mode                 | treasury->member  | member->treasury
-- ---------------------|-------------------|-------------------
-- points_allocation    | issuance_grant    | auction_settlement
-- distributed_clearing | --                | --
-- deferred_payment     | --                | auction_settlement
-- prepaid_credits      | credit_purchase   | auction_settlement
--
-- member->member: transfer, unless it's fram a distributed_clearing auction
--
-- Account handling on user deletion
--
-- Accounts are NOT closed when users delete their accounts or leave. Instead:
-- - Account remains intact with full transaction history
-- - Balance stays visible to community leaders
-- - Community can later transfer balance or absorb it as needed
-- - If user rejoins, they can reconnect to existing account
CREATE TYPE ENTRY_TYPE AS ENUM (
    'issuance_grant',
    'credit_purchase',
    'auction_settlement',
    'transfer'
);

-- Entries in the ledger
-- Each entry has legs in journal_lines with amounts that sum to 0
CREATE TABLE journal_entries (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id      UUID NOT NULL REFERENCES communities (id)
        ON DELETE CASCADE,
    entry_type        ENTRY_TYPE NOT NULL,
    idempotency_key   UUID NOT NULL,
    auction_id        UUID REFERENCES auctions (id),
    -- User who initiated this entry (for treasury/admin operations)
    -- NULL for member-to-member transfers (implicit from account)
    initiated_by_id   UUID REFERENCES users (id)
        ON DELETE SET NULL,
    -- Optional user-provided description
    note              VARCHAR(100),
    created_at        TIMESTAMPTZ NOT NULL,
    UNIQUE (idempotency_key),
    CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL)
);
-- No metadata JSONB; when new metadata forms are needed, add as cols

CREATE TABLE journal_lines (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id          UUID NOT NULL REFERENCES journal_entries (id)
        ON DELETE CASCADE,
    account_id        UUID NOT NULL REFERENCES accounts (id),
    amount            NUMERIC(20, 6) NOT NULL
);

-- Application ensures sum of journal lines for each entry_id is 0

-- Create treasury accounts for all existing communities
INSERT INTO accounts (community_id, owner_type, owner_id, created_at)
SELECT id, 'community_treasury', NULL, NOW()
FROM communities;  -- NOW() OK in backfill

-- Create member_main accounts for all existing community members
INSERT INTO accounts (community_id, owner_type, owner_id, created_at)
SELECT community_id, 'member_main', user_id, NOW()
FROM community_members;  -- NOW() OK in backfill
