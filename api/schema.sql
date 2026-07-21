-- Reference schema: the consolidated end state of api/migrations.
--
-- This file is documentation, not a migration -- nothing executes it in
-- production. It exists so the schema can be read as a coherent whole, with
-- columns and tables in logical rather than chronological order.
--
-- `schema_reference_matches_migrations` (api/tests/api/schema_reference.rs)
-- builds one database from the migration sequence and another from this file,
-- then compares their catalogs. Any migration that changes the schema must
-- update this file, or that test fails and names the drifted object.

-- Roles:
-- 'leader',  -- Only one leader
-- 'coleader',  -- Same privileges as leader, but can have multiple
-- 'moderator',  -- Lower-level privileges, but above member
-- 'member'  -- Default membership level
CREATE TYPE ROLE AS ENUM ('member', 'moderator', 'coleader', 'leader');

-- Token actions for email verification and password reset
CREATE TYPE TOKEN_ACTION AS ENUM ('email_verification', 'password_reset');

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
--
-- # Edge Cases
--
-- If the mode is distributed_clearing and there are no active members when
-- auction settlement occurs, the community treasury account is credited. This
-- makes the settlement behave like deferred_payment, and gives the community
-- leaders the opportunity to rectify the distribution after the fact.
CREATE TYPE CURRENCY_MODE AS ENUM (
    'points_allocation',
    'distributed_clearing',
    'deferred_payment',
    'prepaid_credits'
);

-- Currency account types:
-- - 'member_main': a member's personal account
-- - 'community_treasury': the central community account that issues
--   currency or sinks payments when mode is not distributed_clearing
CREATE TYPE ACCOUNT_OWNER_TYPE AS ENUM (
    'member_main',
    'community_treasury'
);

-- Journal entry types
--
-- member->member: transfer (not mode-dependent)
--
-- `treasury_transfer` covers every transfer between the treasury and a
-- member (allowance issuance, credit purchase, debt settlement, correcting a
-- failed distribution, ...). Mode-specific names asserted a *story* about
-- each transfer that often didn't match the actual motivation; the reason now
-- lives in the entry's note.
--
-- Account handling on user deletion
--
-- Accounts are NOT closed when users delete their accounts or leave. Instead:
-- - Account remains intact with full transaction history
-- - Balance stays visible to community leaders
-- - Community can later transfer balance or absorb it as needed
-- - If user rejoins, they can reconnect to existing account
CREATE TYPE ENTRY_TYPE AS ENUM (
    'transfer',
    'treasury_transfer',
    'auction_settlement',
    'balance_reset',
    'orphaned_account_transfer',
    -- Balanced per-community adjustment quantizing balances to the
    -- community's currency_minor_units: it collects each account's
    -- sub-grain dust so subsequent activity runs at the coarser grain.
    -- Written whenever minor_units is coarsened (in the same transaction,
    -- before the settings change, so the adjustment's lines sit on the
    -- outgoing finer grain), and once at the feature's rollout by the
    -- since-retired dust migration -- only those historical entries carry
    -- lines finer than the minor units declared at the time they were
    -- written, since that dust predated quantization enforcement.
    'rounding_adjustment'
);

-- Subscription tiers
CREATE TYPE SUBSCRIPTION_TIER AS ENUM ('paid');

-- Subscription statuses
CREATE TYPE SUBSCRIPTION_STATUS AS ENUM (
    'active',
    'past_due',
    'canceled',
    'unpaid'
);

-- Billing intervals
CREATE TYPE BILLING_INTERVAL AS ENUM ('month', 'year');

CREATE DOMAIN AMOUNT AS NUMERIC(20, 6) CHECK (VALUE >= 0);

CREATE TABLE communities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    -- Whether new members are active (eligible for distributions) by default.
    new_members_default_active BOOLEAN NOT NULL DEFAULT true,
    currency_mode CURRENCY_MODE NOT NULL DEFAULT 'distributed_clearing',
    -- The default credit limit given to members within a community.
    -- Can be overridden on a per-member basis.
    -- Null means there is no limit.
    default_credit_limit AMOUNT,
    -- User-assigned name and symbol to currency
    currency_name VARCHAR(50) NOT NULL DEFAULT 'dollars',
    currency_symbol VARCHAR(5) NOT NULL DEFAULT '$',
    -- Number of decimal places for display (e.g., 2 for cents, 0 for
    -- whole units)
    currency_minor_units SMALLINT NOT NULL DEFAULT 2
        CHECK (currency_minor_units >= 0 AND currency_minor_units <= 6),
    -- Whether debts can be called for settlement in denominated unit
    debts_callable BOOLEAN NOT NULL DEFAULT true,
    -- Whether ordinary members can see all member balances/limits
    -- Coleaders/leaders always see all; this affects member visibility
    balances_visible_to_members BOOLEAN NOT NULL DEFAULT true,
    -- Allowance settings for points_allocation
    allowance_amount AMOUNT,
    allowance_period INTERVAL,
    -- Starting point for automated issuance
    allowance_start TIMESTAMPTZ,
    -- Stripe customer ID. Persisted at checkout creation so it survives
    -- missed webhooks. NULL for communities that have never started a
    -- checkout.
    stripe_customer_id TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    -- Points allocation constraints
    CHECK (currency_mode != 'points_allocation' OR (
        debts_callable = false
        AND default_credit_limit IS NOT NULL
        AND default_credit_limit = 0
        AND allowance_amount IS NOT NULL
        AND allowance_period IS NOT NULL
        AND allowance_start IS NOT NULL
    )),
    -- Distributed clearing and deferred payment constraints
    CHECK (
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
    CHECK (currency_mode != 'prepaid_credits' OR (
        default_credit_limit IS NOT NULL
        AND default_credit_limit = 0
        AND allowance_amount IS NULL
        AND allowance_period IS NULL
        AND allowance_start IS NULL
    ))
);

-- Case-insensitive uniqueness and lookups for user identifiers.
--
-- The username and email values are preserved exactly as the user entered
-- them. For emails this matters because RFC 5321 leaves the local part
-- (before the @) case-sensitive and owned by the receiving mail server, so
-- preserving the original casing ensures mail is delivered to the address the
-- user actually registered. For usernames it preserves the display casing the
-- user chose.
--
-- Uniqueness and lookups operate on the generated, lowercased "normalized"
-- form rather than the raw value. Generated columns are derived by the
-- database, so the normalized form can never drift from its source value
-- regardless of which write path inserts or updates the row.
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL,
    username_normalized VARCHAR(50)
        GENERATED ALWAYS AS (lower(username)) STORED,
    email VARCHAR(255) NOT NULL,
    email_normalized VARCHAR(255)
        GENERATED ALWAYS AS (lower(email)) STORED,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(255),
    -- If email ownership has been verified; part of signup flow, required to
    -- create or join communities
    email_verified BOOLEAN NOT NULL DEFAULT false,
    -- Set when a user with auction history deletes their account: PII is
    -- anonymized and the row preserved to maintain referential integrity and
    -- distinguish between different deleted users in that history. Also
    -- prevents login. Users without auction history are fully deleted.
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- The unique indexes are partial on `deleted_at IS NULL`, matching the
-- `idx_users_deleted_at` index and the `deleted_at IS NULL` filter that every
-- user lookup already applies. Soft-deleted rows are anonymized to unique
-- per-id values (`deleted-<id>@deleted.local`, `deleted-<id>`), so excluding
-- them keeps the index covering only the live identifier namespace rather than
-- indexing rows that can never be matched against.
CREATE UNIQUE INDEX users_username_normalized_key
ON users (username_normalized)
WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX users_email_normalized_key
ON users (email_normalized)
WHERE deleted_at IS NULL;

-- Index for efficient filtering of non-deleted records
CREATE INDEX idx_users_deleted_at ON users (deleted_at)
WHERE deleted_at IS NULL;

-- Tokens are emailed and are specific to 'email_verification', 'password_reset'
-- actions.
CREATE TABLE tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), -- the token
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    action TOKEN_ACTION NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false, -- can only be used once
    expires_at TIMESTAMPTZ NOT NULL, -- must be used before expiry
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE community_members (
    -- Cascade: if a community is deleted, memberships are deleted too
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role ROLE NOT NULL,
    -- An inactive member is ineligible to receive distributions.
    -- Can be set automatically by community_membership_schedule if user matches
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (community_id, user_id)
);

CREATE UNIQUE INDEX one_leader_per_community
ON community_members (community_id)
WHERE role = 'leader';

CREATE TABLE community_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    -- If provided, the accepting user email must match. Otherwise it's an open
    -- invite to anyone with the invite id.
    email VARCHAR(255),
    -- Invites match against a user's email, so the match compares normalized
    -- forms on both sides. Nullable, matching the nullable `email`.
    email_normalized VARCHAR(255)
        GENERATED ALWAYS AS (lower(email)) STORED,
    single_use BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- A future schedule of community membership that results in automatic
-- updating of the `is_active` state.
--
-- There can be multiple entries for a given email address if membership comes
-- and goes. If a user email is not present in the schedule, activity state is
-- only manually configured.
--
-- The email field can be an ordinary string or a hex digest of the SHA256 of
-- the email. Both are checked. Hashing reduces the privacy loss of users that
-- have not yet voluntarily signed up, but that are included in a community
-- schedule.
CREATE TABLE community_membership_schedule (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    email VARCHAR(255) NOT NULL,  -- email identifier
    -- The schedule joins its email against users.email, so the join (and the
    -- per-row activity update) compares normalized forms: a scheduled
    -- `Bob@x.com` matches a registered `bob@x.com`. The hash-or-raw design
    -- documented above is unaffected; only the raw-email matching path is
    -- case-insensitive.
    email_normalized VARCHAR(255)
        GENERATED ALWAYS AS (lower(email)) STORED,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Auction parameters are immutable and copy-on-write if they are used in a
-- past auction.
CREATE TABLE auction_params (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Length of time of each round.
    round_duration INTERVAL NOT NULL,
    -- 20 digits total, with 6 units of precision
    bid_increment NUMERIC(20, 6) NOT NULL,
    -- Eligibility requirements as the auction progresses. Determines each
    -- round's eligibility_threshold
    activity_rule_params JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Open hours for a site when possession takes place. Can be used for holidays
-- by updating the open hours within a week of the closure.
--
-- Maps 1-1 with a site, so when the site's open hours are updated these hours
-- get updated
CREATE TABLE open_hours (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

-- If a day of the week is absent, the site is assumed to be closed that day.
CREATE TABLE open_hours_weekday (
    open_hours_id UUID NOT NULL REFERENCES open_hours (id) ON DELETE CASCADE,
    -- 1 = Monday, 7 = Sunday
    day_of_week SMALLINT NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
    open_time TIME NOT NULL,  -- Local time
    close_time TIME NOT NULL, -- Local time (if before open_time, is next day)
    PRIMARY KEY (open_hours_id, day_of_week)
);

-- Images for sites or spaces.
CREATE TABLE site_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    image_data BYTEA NOT NULL,
    mime_type VARCHAR(50) NOT NULL DEFAULT 'image/jpeg',
    file_size BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (community_id, name)
);

-- Added out of line because communities and site_images reference each other,
-- so one of the two directions can't be declared in the table body.
ALTER TABLE communities
ADD COLUMN community_image_id UUID REFERENCES site_images (id);

-- A location consisting of indivisible spaces available for rent, and for
-- which auctions take place.
CREATE TABLE sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    default_auction_params_id UUID NOT NULL REFERENCES auction_params (id),

    -- Auction auto scheduling parameters --

    -- Duration of possession and period between auctions.
    possession_period INTERVAL NOT NULL,
    -- Amount of time before the change in possession that the auction begins.
    auction_lead_time INTERVAL NOT NULL,
    -- Amount of time before the start of auction that the auction row exists
    -- and proxy bids can be prepared.
    proxy_bidding_lead_time INTERVAL NOT NULL,
    -- If not present, the site is assumed to be open all the time.
    open_hours_id UUID REFERENCES open_hours (id) ON DELETE SET NULL,

    -- Whether this site is automatically scheduled for auction. Otherwise
    -- auctions are manually triggered.
    auto_schedule BOOLEAN NOT NULL DEFAULT true,

    -- IANA time zone, e.g. 'America/Los_Angeles'.
    -- If not provided, datetime math uses UTC and the times render in the
    -- users's local time.
    timezone TEXT,

    -- Image is optional if the location is otherwise well-described.
    site_image_id UUID REFERENCES site_images (id) ON DELETE SET NULL,
    -- Soft delete (default): hides from UI, preserves all auction history.
    -- Use when deprecating a site that's no longer in use. A hard delete
    -- cascades to spaces, auctions, and all history, and is used when
    -- intentionally removing all trace of a site.
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (community_id, name)
);

CREATE INDEX idx_sites_deleted_at ON sites (deleted_at)
WHERE deleted_at IS NULL;

-- An individual space available for possession.
CREATE TABLE spaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    eligibility_points DOUBLE PRECISION NOT NULL,
    -- Whether this space is available for auction, which can be changed based
    -- on bundling.
    is_available BOOLEAN NOT NULL DEFAULT true,
    -- Positive = normal reserve (minimum starting bid). Negative = chore
    -- semantics (winner is paid to take on the obligation). No CHECK
    -- constraint -- negatives are valid. Cannot reuse the AMOUNT domain
    -- because that is constrained `>= 0`.
    reserve_price NUMERIC(20, 6) NOT NULL DEFAULT 0,
    -- Image is optional if the location is otherwise well-described.
    site_image_id UUID REFERENCES site_images (id) ON DELETE SET NULL,
    -- Soft delete (default): hides from UI, preserves auction history
    -- referencing this space. A hard delete cascades to auction history; the
    -- application checks for auction history before allowing one.
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (eligibility_points >= 0.0)
);

CREATE INDEX idx_spaces_deleted_at ON spaces (deleted_at)
WHERE deleted_at IS NULL;

-- Space names are unique only among non-deleted spaces, so a name can be
-- reused after soft-delete (copy-on-write).
CREATE UNIQUE INDEX spaces_site_id_name_unique
ON spaces (site_id, name)
WHERE deleted_at IS NULL;

CREATE TABLE auctions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    -- The specific possession period being auctioned.
    possession_start_at TIMESTAMPTZ NOT NULL,
    possession_end_at TIMESTAMPTZ NOT NULL,
    -- Start and end times of the auction. A NULL start_at means the auction is
    -- waiting to be started manually (or scheduled later) by a coleader+; the
    -- scheduler ignores such auctions.
    start_at TIMESTAMPTZ,
    end_at TIMESTAMPTZ, -- Filled in when the auction completes.
    -- Soft-delete-style cancellation. A canceled auction has end_at set (which
    -- stops the scheduler from processing it further, including settlement) and
    -- was_canceled = TRUE so the UI can distinguish cancellation from a normal
    -- conclusion. Canceled auctions never get a settlement journal entry, so
    -- they remain hard-deletable (journal_entries.auction_id is ON DELETE
    -- RESTRICT for settled auctions).
    was_canceled BOOLEAN NOT NULL DEFAULT FALSE,
    -- The auction params used in this auction.
    auction_params_id UUID NOT NULL REFERENCES auction_params (id),
    -- Scheduler failure tracking for debugging and backoff
    scheduler_failure_count INTEGER NOT NULL DEFAULT 0,
    scheduler_last_failed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Not directly editable by users (only read/list), since the system manages
-- the auction rounds itself.
CREATE TABLE auction_rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    -- The index of the round in the auction, starting at 0.
    round_num INTEGER NOT NULL,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    -- Fraction of the bidder's eligibility that must be met, e.g. 80%
    eligibility_threshold DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (auction_id, round_num),
    -- Elibility requirements can be 0% or 100% of current user eligibility.
    -- 0% means no eligibility is required, whereas 100% prevents any demand
    -- shifting to higher-value spaces.
    CHECK (eligibility_threshold >= 0.0 AND eligibility_threshold <= 1.0)
);

-- The current winner (until the next round) of a space.
--
-- Populated when a round concludes. May not exist for a space that has no
-- bidding activity. Once someone bids for a space, results exist for that
-- space through to the end of the auction, since spaces can only be
-- relinquished when outbid.
CREATE TABLE round_space_results (
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    winning_user_id UUID REFERENCES users (id) NOT NULL,
    -- space value at the conclusion of this round
    value NUMERIC(20, 6) NOT NULL,
    PRIMARY KEY (space_id, round_id)
);
CREATE INDEX idx_round_space_results_space_id ON round_space_results (space_id);
CREATE INDEX idx_round_space_results_round_id ON round_space_results (round_id);
CREATE INDEX idx_round_space_results_round_space ON round_space_results (
    round_id, space_id
);

-- All bids for spaces in an auction round that meet (are) the minimum bid
-- increment.
--
-- A user must have the necessary balance to place a bid.
--
-- At the end of a round one of the valid bidders is chosen randomly as the
-- round winner.
CREATE TABLE bids (
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (space_id, round_id, user_id)
);
CREATE INDEX idx_bids_user_id ON bids (user_id);
CREATE INDEX idx_bids_round_id ON bids (round_id);
CREATE INDEX idx_bids_space_id ON bids (space_id);
CREATE INDEX idx_bids_round_id_user_id ON bids (round_id, user_id);

-- User eligibility across auction rounds.
--
-- Like round_space_results, this is updated after a round concludes, and
-- indicates how much eligibility the user has for the next round, based on the
-- previous round's eligibility threshold and bidder activity. Only exists for
-- round numbers greater than zero.
CREATE TABLE user_eligibilities (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    eligibility DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (user_id, round_id),
    CHECK (eligibility >= 0)
);

-- User-assigned values for each space, for proxy bidding.
--
-- The proxy bidding mechanism automatically bids for the space that has the
-- largest difference between the user's value and the current value. This is a
-- simple utility-greedy strategy that ignores the value of bundles of items
-- (complementary demand).
--
-- `value` may be negative so chore valuations can be expressed directly (e.g.,
-- "I'll accept down to -$5 for this chore").
CREATE TABLE user_values (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    value NUMERIC(20, 6) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, space_id)
);
CREATE INDEX idx_user_values_user_id ON user_values (user_id);

-- A row present in this table indicates that a user's space values should be
-- used for automatic proxy bidding.
--
-- max_items defines how many items the user is willing to win. The proxy
-- bidding system will bid for up to that many items, attempting to maximize
-- the user's surplus (max_value - current_price).
CREATE TABLE use_proxy_bidding (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    max_items INTEGER NOT NULL,
    -- Writer-side dirty flag: set TRUE in the writer's own transaction by
    -- proxy settings and user-value saves; cleared only by the processor's
    -- claim transaction. The flag derives re-selection ordering from the
    -- database's own serialization instead of clock comparisons.
    needs_processing BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, auction_id)
);
CREATE INDEX idx_use_proxy_bidding_user_id ON use_proxy_bidding (user_id);
CREATE INDEX idx_use_proxy_bidding_auction_id ON use_proxy_bidding (auction_id);
CREATE INDEX idx_use_proxy_bidding_user_id_auction_id ON use_proxy_bidding
(user_id, auction_id);

-- Per-(round, user) processing marker. An explicit marker row is needed
-- because "processed, but no surplus so zero bids" is indistinguishable
-- from "unprocessed" via bids alone. processed_at is informational;
-- re-selection is driven by marker existence (per-round baseline), the
-- needs_processing flag (mid-round change), and failure backoff. A
-- marker can exist with processed_at NULL when the first attempt fails.
CREATE TABLE proxy_round_processing (
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    processed_at TIMESTAMPTZ,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_failed_at TIMESTAMPTZ,
    PRIMARY KEY (round_id, user_id)
);

-- Ledger

CREATE TABLE accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id    UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    owner_type      ACCOUNT_OWNER_TYPE NOT NULL,
    -- Usually this cascade is blocked by the ON DELETE RESTRICT in
    -- journal_lines if the user has transaction history.
    owner_id        UUID REFERENCES users (id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL,
    -- Materialized balance kept in sync by application
    -- Positive = credit balance, Negative = debt
    balance_cached         NUMERIC(20, 6) NOT NULL DEFAULT 0,
    -- Credit limit override for this account
    -- NULL = use community default_credit_limit
    -- Only applies to member_main accounts; treasury has no limit
    credit_limit_override  AMOUNT,
    -- Application enforces: balance_cached >=
    --   -COALESCE(credit_limit_override, community.default_credit_limit, infinity)
    -- For member_main accounts, owner_id must be set
    -- For community_treasury accounts, owner_id must be null
    CHECK ((owner_type = 'member_main' AND owner_id IS NOT NULL) OR
           (owner_type = 'community_treasury' AND owner_id IS NULL))
);

-- Efficient and unique account lookups by owner
CREATE UNIQUE INDEX idx_accounts_owner
ON accounts (community_id, owner_type, owner_id);

-- Entries in the ledger
-- Each entry has legs in journal_lines with amounts that sum to 0
--
-- ## Deletion semantics
--
-- The ledger defines what must be preserved. FKs use ON DELETE RESTRICT to
-- block deletion of referenced rows:
-- - journal_entries.auction_id → blocks auction/site deletion
-- - journal_lines.account_id → blocks account/user deletion
--
-- This means:
-- - Users with transaction history are anonymized, not deleted
-- - Sites with auction settlements cannot be hard-deleted
-- - Community deletion explicitly deletes journal_entries first to unblock
--   the cascade (the one case where we destroy financial history)
CREATE TABLE journal_entries (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id      UUID NOT NULL REFERENCES communities (id)
        ON DELETE CASCADE,
    entry_type        ENTRY_TYPE NOT NULL,
    idempotency_key   UUID NOT NULL,
    auction_id        UUID REFERENCES auctions (id) ON DELETE RESTRICT,
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
    account_id        UUID NOT NULL REFERENCES accounts (id) ON DELETE RESTRICT,
    amount            NUMERIC(20, 6) NOT NULL
);

-- Efficient transaction history queries by account
CREATE INDEX idx_journal_lines_account_id ON journal_lines (account_id);

-- Application ensures sum of journal lines for each entry_id is 0

-- Billing

-- Paid community subscriptions (missing row = free tier).
--
-- All columns are upserted from
-- customer.subscription.{created,updated,deleted} webhooks.
-- The community_id is resolved by fetching the Stripe
-- customer's metadata on the first event for a subscription.
--
-- Effective tier: active/past_due = paid,
--                 canceled/unpaid/no row = free.
CREATE TABLE community_subscriptions (
    community_id UUID PRIMARY KEY
        REFERENCES communities (id) ON DELETE CASCADE,
    -- Always 'paid'; column exists so the enum can be
    -- extended if we add more tiers later.
    tier SUBSCRIPTION_TIER NOT NULL,
    -- Mirrors the Stripe subscription status. All updates come from
    -- subscription webhook events.
    status SUBSCRIPTION_STATUS NOT NULL,
    -- 'month' or 'year'. From the subscription's price.recurring.interval.
    billing_interval BILLING_INTERVAL NOT NULL,
    -- Stripe subscription ID. Used to correlate webhook events to this row.
    stripe_subscription_id TEXT NOT NULL UNIQUE,
    -- Current billing period boundaries. Sourced from the Stripe subscription
    -- object on each update.
    current_period_start TIMESTAMPTZ NOT NULL,
    current_period_end TIMESTAMPTZ NOT NULL,
    -- True when the customer has chosen to cancel but the current period
    -- hasn't ended yet.
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
    -- When the subscription was canceled. Sourced from the Stripe
    -- subscription's canceled_at field.
    canceled_at TIMESTAMPTZ,
    -- Row creation time (first checkout.session.completed).
    created_at TIMESTAMPTZ NOT NULL,
    -- Last time any webhook updated this row.
    updated_at TIMESTAMPTZ NOT NULL
);

-- Cached storage calculations
CREATE TABLE community_storage_usage (
    community_id UUID PRIMARY KEY REFERENCES communities (id) ON DELETE CASCADE,
    image_bytes BIGINT NOT NULL,
    member_bytes BIGINT NOT NULL,
    space_bytes BIGINT NOT NULL,
    auction_bytes BIGINT NOT NULL,
    transaction_bytes BIGINT NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL
);

-- Audit log

-- Track who changed what and when.
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID, -- the user who performed the action
    action TEXT NOT NULL, -- e.g., 'update_role', 'place_bid'
    target_table TEXT,
    target_id UUID,
    details JSONB, -- anything relevant: old/new values, diffs, etc.
    created_at TIMESTAMPTZ NOT NULL
);
