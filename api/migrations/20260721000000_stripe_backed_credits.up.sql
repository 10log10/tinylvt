-- Schema for Stripe-backed credits (the stripe-auction-payments
-- changeset): connected accounts, saved-card metadata, per-auction
-- funding state, the stripe_payment ledger linkage, the notification
-- outbox, credit purchases, and reconciliation state.

-- The mode's defining invariant is that every commitment is backed by
-- real funds -- a credit balance or a card authorization -- so
-- 'backed_credits' names it better than 'prepaid_credits' did: the card
-- side is authorized, not prepaid. No community uses the old value yet.
-- RENAME VALUE keeps the enum's sort order and rewrites dependent CHECK
-- constraints by OID.
ALTER TYPE CURRENCY_MODE RENAME VALUE 'prepaid_credits' TO 'backed_credits';

-- Community's connected Stripe account. Card charges are direct charges
-- on this account; the platform never holds funds.
ALTER TABLE communities
    ADD COLUMN stripe_account_id TEXT UNIQUE,
    -- Mirrors the account's charges_enabled capability from account
    -- webhooks; gates card-backed bidding.
    ADD COLUMN stripe_charges_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    -- Community disconnected the platform; cleared on reconnect.
    ADD COLUMN stripe_deauthorized_at TIMESTAMPTZ,
    -- Last-run watermark for the hourly reconciliation pass. When any
    -- community's watermark is older than the interval (or NULL), one
    -- pass runs for ALL communities and restamps every row -- a single
    -- hourly pass with one summary log line, deduplicated across
    -- instances by an advisory lock rather than staggered per-community
    -- staleness. Set current at creation: because that gate is global,
    -- treating a new community (which has nothing to reconcile) as due
    -- would force a full unscheduled pass over every community.
    ADD COLUMN last_reconciliation_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

-- The default only backfills existing rows above; the column is
-- application-supplied from TimeSource, so leaving it would let an
-- insert silently bypass time mocking.
ALTER TABLE communities
    ALTER COLUMN last_reconciliation_at DROP DEFAULT;

-- The member's affirmative, revocable grant allowing this community to
-- charge their saved card when bids exceed balance (merchant-initiated
-- holds/charges only; member-initiated Checkout purchases don't need
-- it). NULL = no grant. Revocation only stops new authorizations and
-- raises; existing holds back binding bids and release via
-- settlement/cancel.
ALTER TABLE community_members
    ADD COLUMN card_charges_granted_at TIMESTAMPTZ;

-- Authorization sizing strategy (platform-wide member setting). TRUE =
-- budget holds: authorizations cover the member's auction budget (the
-- sum of their max_items largest user values among the auction's
-- spaces, net of available balance) -- fully member-determined, one
-- statement line, and no unattended mid-auction raises in the common
-- case. FALSE = start at the denomination's minimum charge and raise
-- catch-up-or-double as bids require. Either way, raises use
-- swap-reauth (manual bids, mid-auction value changes, and stale
-- budgets fall back to it).
ALTER TABLE users
    ADD COLUMN budget_holds BOOLEAN NOT NULL DEFAULT TRUE;

-- Display metadata only; the card lives in Stripe. One platform-level
-- Stripe customer per user, cloned to connected accounts at charge time.
CREATE TABLE user_payment_profiles (
    user_id            UUID PRIMARY KEY REFERENCES users (id)
                           ON DELETE CASCADE,
    stripe_customer_id TEXT NOT NULL UNIQUE,
    payment_method_id  TEXT,        -- NULL until a card is saved
    card_brand         TEXT,
    card_last4         TEXT,
    card_exp_month     SMALLINT,
    card_exp_year      SMALLINT,
    consented_at       TIMESTAMPTZ, -- merchant-initiated-charge consent
    created_at         TIMESTAMPTZ NOT NULL,
    updated_at         TIMESTAMPTZ NOT NULL,
    -- The card columns are written all-or-nothing (save sets all five,
    -- remove/detach clears all five); enforced so "card saved" checks
    -- can gate on payment_method_id alone while display reads all four
    -- metadata columns.
    CONSTRAINT user_payment_profiles_card_columns_check CHECK (
        num_nonnulls(payment_method_id, card_brand, card_last4,
                     card_exp_month, card_exp_year) IN (0, 5)
    )
);

-- Out-of-band detach: the payment_method.detached webhook clears the
-- stored card by payment method id, not by user.
CREATE INDEX idx_user_payment_profiles_method
    ON user_payment_profiles (payment_method_id)
    WHERE payment_method_id IS NOT NULL;

-- Status lifecycle. Non-terminal states are worker instructions or
-- in-flight markers; terminal states are Stripe facts:
--   pending          -> authorized (confirm); stale pendings are reused
--                       as the idempotency-key seed, never transitioned
--   checkout_created -> authorized (session paid, adoption webhook) |
--                       canceled (session expired/abandoned/superseded)
--   authorized       -> capture_pending | release_pending |
--                       superseded (swap flip) | expired | canceled
--   capture_pending  -> captured | failed
--   release_pending  -> canceled (worker cancel or webhook)
--   superseded       -> canceled (release_pending with a reason:
--                       replaced by a live intent)
-- Terminal: captured, canceled, expired, failed. "Was replaced"
-- survives cancellation in the lineage (replaces_intent_id on the
-- replacement row) -- status is the Stripe object's current state,
-- lineage is history.
CREATE TYPE FUNDING_INTENT_STATUS AS ENUM (
    'pending',
    'checkout_created',
    'authorized',
    'capture_pending',
    'captured',
    'release_pending',
    'superseded',
    'canceled',
    'expired',
    'failed'
);

-- Who initiated the authorization: scopes age-cancel notifications
-- (system auths re-auth silently, member auths notify once) and labels
-- hold history ("automatic pre-authorization" vs "you authorized $50").
-- 'member_checkout' is the unsaved-card flow: a member-present Checkout
-- session (mode=payment, manual capture) with nothing stored -- no
-- saved card, no card-charge grant required.
CREATE TYPE FUNDING_INTENT_ORIGIN AS ENUM (
    'scheduled_preauth',
    'member_preauth',
    'bid_flow',
    'member_checkout'
);

-- Mirror rows for manual-capture PaymentIntents backing one member's
-- bids in one auction. Canceled/superseded rows are kept for
-- hold-history UX and dispute linkage.
CREATE TABLE funding_intents (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auction_id            UUID NOT NULL REFERENCES auctions (id)
                              ON DELETE CASCADE,
    -- RESTRICT: a user hard delete runs no wind-down, and a cascade
    -- here could erase the only record of a live Stripe hold.
    -- delete_user's FK fallback anonymizes instead, preserving the
    -- rows and releasing live holds.
    user_id               UUID NOT NULL REFERENCES users (id)
                              ON DELETE RESTRICT,
    payment_intent_id     TEXT UNIQUE,  -- NULL during pending pre-insert
    is_active             BOOLEAN NOT NULL DEFAULT FALSE,
    status                FUNDING_INTENT_STATUS NOT NULL
                              DEFAULT 'pending',
    origin                FUNDING_INTENT_ORIGIN NOT NULL,
    -- The sized amount the pending order will request at Stripe, set
    -- at insert and immutable — the execute step replays it, so a
    -- crashed attempt replays identical parameters (idempotent replay
    -- at Stripe). For checkout rows: the session's authorization
    -- amount (the member's chosen cap).
    requested_amount      AMOUNT NOT NULL,
    -- Unsaved-card flow only: the Checkout session minted for this
    -- row, set after the mint (the row is pre-inserted so its id can
    -- seed the session's idempotency key and metadata).
    checkout_session_id   TEXT UNIQUE,
    authorized_amount     AMOUNT,       -- Stripe truth, set at authorization
    capture_amount        AMOUNT,       -- target, set at conclusion
    capture_before        TIMESTAMPTZ,  -- auth expiry
    last_decline_at       TIMESTAMPTZ,
    last_decline_code     TEXT,
    -- Set once at insert on a swap replacement row, immutable; the
    -- lineage chain powers hold-history UX.
    replaces_intent_id    UUID REFERENCES funding_intents (id),
    -- Pace capture/cancel retries (scheduler failure-backoff style).
    worker_failure_count  SMALLINT NOT NULL DEFAULT 0,
    worker_last_failed_at TIMESTAMPTZ,
    authorized_at         TIMESTAMPTZ,
    -- Set by the orphaned-hold sweep once the row's Stripe-side state
    -- has been resolved (a recovered hold canceled, or its absence
    -- confirmed), so each canceled no-PI row is examined once.
    reconciled_at         TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL,
    updated_at            TIMESTAMPTZ NOT NULL,
    -- Post-authorization statuses always carry the Stripe facts set at
    -- activation (superseded/release_pending only transition from
    -- authorized rows and null nothing), and capture_pending
    -- additionally carries the capture target. Code decodes these as
    -- non-optional (settlement, reconciliation's overdue-capture check).
    CONSTRAINT funding_intents_authorized_columns_check CHECK (
        status NOT IN ('authorized', 'superseded', 'release_pending',
                       'capture_pending')
        OR num_nonnulls(payment_intent_id, authorized_amount,
                        capture_before) = 3
    ),
    CONSTRAINT funding_intents_capture_amount_check CHECK (
        status != 'capture_pending' OR capture_amount IS NOT NULL
    ),
    -- Checkout sessions and the checkout_created status belong to the
    -- unsaved-card flow exclusively.
    CONSTRAINT funding_intents_checkout_origin_check CHECK (
        (checkout_session_id IS NULL AND status != 'checkout_created')
        OR origin = 'member_checkout'
    )
);

-- One active intent per (member, auction).
CREATE UNIQUE INDEX idx_funding_intents_active
    ON funding_intents (auction_id, user_id) WHERE is_active;
-- One pending row per (member, auction): concurrent creation
-- pre-inserts collide here, so the survivor is every retry's key seed
-- and replay-by-key is deterministic.
CREATE UNIQUE INDEX idx_funding_intents_pending
    ON funding_intents (auction_id, user_id) WHERE status = 'pending';
-- One open checkout per (member, auction): a replacement mint retires
-- the predecessor (expiring its session at Stripe) before inserting.
-- Also serves the reconciliation sweep's aged-checkout scan.
CREATE UNIQUE INDEX idx_funding_intents_checkout
    ON funding_intents (auction_id, user_id)
    WHERE status = 'checkout_created';
-- Worker selection and expiry/age scans.
CREATE INDEX idx_funding_intents_worker ON funding_intents (status)
    WHERE status IN ('pending', 'capture_pending', 'release_pending',
                     'superseded');
CREATE INDEX idx_funding_intents_expiry
    ON funding_intents (capture_before) WHERE is_active;
-- Latest-intent lookup (newest row per (member, auction)) and cascade
-- deletes. The partial indexes above only cover active/pending rows,
-- and terminal rows are retained forever by design.
CREATE INDEX idx_funding_intents_latest
    ON funding_intents (auction_id, user_id, created_at DESC);
-- Member-across-auctions scans (live-auth aggregation on every backed
-- gate, pending-capture aggregation, departure release); community
-- scoping joins auctions -> sites.
CREATE INDEX idx_funding_intents_member
    ON funding_intents (user_id);
-- Self-FK integrity checks when intent rows are deleted (the auction
-- cascade); without this each deleted row seq-scans
-- for referencing replacement rows.
CREATE INDEX idx_funding_intents_replaces
    ON funding_intents (replaces_intent_id)
    WHERE replaces_intent_id IS NOT NULL;
-- Matches the orphaned-hold sweep's selection exactly; near-empty in
-- steady state since the sweep stamps reconciled_at as it resolves
-- each row.
CREATE INDEX idx_funding_intents_orphan_sweep
    ON funding_intents (created_at)
    WHERE status = 'canceled' AND payment_intent_id IS NULL
      AND reconciled_at IS NULL;

-- New 'stripe_payment' entry type: a capture or purchase recorded from
-- a Stripe PaymentIntent. Splits back out of the collapsed
-- treasury_transfer consistently with the collapse rationale: the old
-- fine types were human-asserted stories, this one is a
-- machine-recorded fact with an enforced linkage. ADD VALUE plus a
-- CHECK referencing the new value cannot share one transactional
-- migration, so recreate the type (reserve_prices precedent). The
-- auction_settlement CHECK is dropped first and recreated after, since
-- its literal would otherwise resolve against the new type
-- mid-migration.
ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_check;

ALTER TYPE ENTRY_TYPE RENAME TO ENTRY_TYPE_OLD;

CREATE TYPE ENTRY_TYPE AS ENUM (
    'transfer',
    'treasury_transfer',
    'auction_settlement',
    'balance_reset',
    'orphaned_account_transfer',
    'rounding_adjustment',
    'stripe_payment'
);

ALTER TABLE journal_entries
    ALTER COLUMN entry_type TYPE ENTRY_TYPE
    USING entry_type::TEXT::ENTRY_TYPE;

DROP TYPE ENTRY_TYPE_OLD;

ALTER TABLE journal_entries
    ADD CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL);

-- TEXT, no FK: purchases have no funding_intents row, and the permanent
-- ledger must outlive intent rows under the cascade doctrine. Not
-- unique: later refund recording references the same intent as its
-- purchase.
ALTER TABLE journal_entries ADD COLUMN payment_intent_id TEXT;

ALTER TABLE journal_entries
    ADD CHECK (entry_type != 'stripe_payment' OR payment_intent_id IS NOT NULL);

-- Makes intent<->entry linkage bidirectional (the UUIDv5 idempotency
-- key derives from the intent id but is one-way) and reconciliation's
-- intent<->entry matching a plain join.
CREATE INDEX idx_journal_entries_payment_intent
    ON journal_entries (payment_intent_id)
    WHERE payment_intent_id IS NOT NULL;

-- Transactional notification outbox: user-facing emails are enqueued
-- in the same transaction as the state change they announce, and a
-- scheduler loop drains them with retry backoff. The unique dedup_key
-- (a legible deterministic string like 'capture_receipt:{intent_id}')
-- makes enqueue idempotent across claim replays and is the once-only
-- guarantee per event.
CREATE TYPE NOTIFICATION_KIND AS ENUM (
    'card_action_needed',
    'authorization_expiring',
    'capture_receipt',
    'backing_lost',
    'capture_failed',
    'checkout_released'
);

CREATE TABLE notification_outbox (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id        UUID NOT NULL REFERENCES users (id)
                       ON DELETE CASCADE,
    kind           NOTIFICATION_KIND NOT NULL,
    dedup_key      TEXT NOT NULL UNIQUE,
    -- The template's display inputs, snapshotted at enqueue (a
    -- serde-tagged per-kind enum), so a send is a pure function of the
    -- row plus the member's current email address, and rows whose
    -- referents cascade-delete still render.
    params         JSONB NOT NULL,
    sent_at        TIMESTAMPTZ,
    -- Delivery retry pacing (scheduler failure-backoff style).
    failure_count  SMALLINT NOT NULL DEFAULT 0,
    last_failed_at TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL
);

-- Drain selection.
CREATE INDEX idx_notification_outbox_unsent
    ON notification_outbox (created_at) WHERE sent_at IS NULL;

-- Credit purchases (backed_credits mode): a member buys credits, or
-- settles debt from a failed settlement capture, through a Stripe
-- Checkout session charged directly on the community's connected
-- account. The row is workflow state around the Checkout session; the
-- permanent record is the ledger's stripe_payment issuance (no
-- auction_id, unlike captures), created idempotently when
-- payment_intent.succeeded arrives.
--
-- Debt settlement is a distinct kind, not a UI framing: the charge is
-- validated against the member's exact effective debt (balance plus
-- pending captures) so the balance never crosses zero, keeping it
-- payment-for-services-rendered rather than stored value. It therefore
-- stays available while top-up purchases are deployment-gated pending
-- Stripe's stored-value approval, and remains a separate exact-amount
-- action afterward for members who don't want to store value.
CREATE TYPE PURCHASE_KIND AS ENUM ('top_up', 'debt_settlement');

-- 'created' = Checkout session minted, payment not completed (never
-- shown as pending; abandoned sessions move to 'expired' via
-- checkout.session.expired). 'processing' = a delayed method (ACH) is
-- settling — shown as pending, credits not yet issued. Terminal:
-- 'succeeded' (credits issued), 'failed', 'expired'.
CREATE TYPE PURCHASE_STATUS AS ENUM (
    'created',
    'processing',
    'succeeded',
    'failed',
    'expired'
);

CREATE TABLE credit_purchases (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id        UUID NOT NULL REFERENCES communities (id)
                            ON DELETE CASCADE,
    -- RESTRICT: a cascade could erase an in-flight ACH purchase whose
    -- late success must find its row. delete_user's FK fallback
    -- anonymizes instead.
    user_id             UUID NOT NULL REFERENCES users (id)
                            ON DELETE RESTRICT,
    kind                PURCHASE_KIND NOT NULL,
    status              PURCHASE_STATUS NOT NULL DEFAULT 'created',
    -- Face value in the community's denomination: charge amount and
    -- issued credits are identical (fees are absorbed by the
    -- community, never surcharged).
    amount              AMOUNT NOT NULL,
    -- Set after the session is minted (the row is pre-inserted so its
    -- id can seed the session's idempotency key and metadata).
    checkout_session_id TEXT UNIQUE,
    -- Learned from checkout.session.completed or the first
    -- PaymentIntent event carrying our metadata.
    payment_intent_id   TEXT UNIQUE,
    created_at          TIMESTAMPTZ NOT NULL,
    updated_at          TIMESTAMPTZ NOT NULL
);

-- Member's purchase history and pending display.
CREATE INDEX idx_credit_purchases_member
    ON credit_purchases (user_id, community_id, created_at);
