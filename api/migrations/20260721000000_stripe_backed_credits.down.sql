DROP TABLE credit_purchases;
DROP TYPE PURCHASE_STATUS;
DROP TYPE PURCHASE_KIND;

DROP TABLE notification_outbox;
DROP TYPE NOTIFICATION_KIND;

DROP INDEX idx_journal_entries_payment_intent;

-- The stripe_payment CHECK was the second unnamed check added to
-- journal_entries, so it carries the generated name with suffix 1.
ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_check1;

ALTER TABLE journal_entries DROP COLUMN payment_intent_id;

-- Enum values can't be dropped; recreate the type without the value.
-- The USING cast fails loudly if any stripe_payment entries exist --
-- those rows would have to be dealt with manually before downgrading.
ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_check;

ALTER TYPE ENTRY_TYPE RENAME TO ENTRY_TYPE_OLD;

CREATE TYPE ENTRY_TYPE AS ENUM (
    'transfer',
    'treasury_transfer',
    'auction_settlement',
    'balance_reset',
    'orphaned_account_transfer',
    'rounding_adjustment'
);

ALTER TABLE journal_entries
    ALTER COLUMN entry_type TYPE ENTRY_TYPE
    USING entry_type::TEXT::ENTRY_TYPE;

DROP TYPE ENTRY_TYPE_OLD;

ALTER TABLE journal_entries
    ADD CHECK (entry_type != 'auction_settlement' OR auction_id IS NOT NULL);

DROP TABLE funding_intents;
DROP TYPE FUNDING_INTENT_STATUS;
DROP TYPE FUNDING_INTENT_ORIGIN;
DROP TABLE user_payment_profiles;

ALTER TABLE users DROP COLUMN budget_holds;

ALTER TABLE community_members DROP COLUMN card_charges_granted_at;

ALTER TABLE communities
    DROP COLUMN stripe_account_id,
    DROP COLUMN stripe_charges_enabled,
    DROP COLUMN stripe_deauthorized_at,
    DROP COLUMN last_reconciliation_at;

ALTER TYPE CURRENCY_MODE RENAME VALUE 'backed_credits' TO 'prepaid_credits';
