-- Track file sizes on images
ALTER TABLE site_images ADD COLUMN file_size BIGINT;

-- Backfill existing images
UPDATE site_images SET file_size = octet_length(image_data);

-- Make NOT NULL after backfill
ALTER TABLE site_images ALTER COLUMN file_size SET NOT NULL;

-- Stripe customer ID on communities. Persisted at checkout creation
-- so it survives missed webhooks. NULL for communities that have
-- never started a checkout.
ALTER TABLE communities ADD COLUMN stripe_customer_id TEXT UNIQUE;

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
        REFERENCES communities(id) ON DELETE CASCADE,
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
    community_id UUID PRIMARY KEY REFERENCES communities(id) ON DELETE CASCADE,
    image_bytes BIGINT NOT NULL,
    member_bytes BIGINT NOT NULL,
    space_bytes BIGINT NOT NULL,
    auction_bytes BIGINT NOT NULL,
    transaction_bytes BIGINT NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL
);
