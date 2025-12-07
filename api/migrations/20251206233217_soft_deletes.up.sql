-- Soft Deletes and Referential Integrity for Auction History
--
-- This migration introduces soft deletes for sites and spaces, anonymization
-- for users, and adjusts foreign key constraints to preserve auction history
-- integrity.
--
-- ## Design Rationale
--
-- ### Users
-- When a user deletes their account:
-- - If they have auction history (bids, round_space_results, or
--   user_eligibilities), their PII is anonymized, `deleted_at` is set, and the
--   row is preserved to maintain referential integrity and allow distinguishing
--   between different deleted users in auction history. The `deleted_at` marker
--   prevents login and hides the user from member lists. Non-historical data
--   (user_values, use_proxy_bidding, tokens, community_members) is deleted.
-- - If they have no auction history, the row can be fully deleted.
--
-- ### Sites
-- Sites support both soft and hard deletes:
-- - Soft delete (default): Sets `deleted_at`, hides from UI, preserves all
--   auction history. Use when deprecating a site that's no longer in use.
-- - Hard delete (explicit): Cascades to spaces, auctions, and all history.
--   Use when intentionally removing all trace of a site.
--
-- ### Spaces
-- Spaces support both soft and hard deletes:
-- - Soft delete (default): Sets `deleted_at`, hides from UI, preserves auction
--   history referencing this space.
-- - Hard delete (explicit): Only allowed if no auctions reference the space.
--   The RESTRICT constraint ensures auction history integrity—users must first
--   hard delete any auctions that include this space.
--
-- The `round_space_results` and `bids` tables use ON DELETE RESTRICT for
-- space_id to enforce this. These are part of the auction history that must
-- be preserved if the auction exists.
--
-- The `user_values` table keeps ON DELETE CASCADE since it represents the
-- user's current valuations for extant spaces, not historical auction data.
-- Future work may snapshot valuations at auction time.
--
-- ### Auctions
-- Hard delete cascades to rounds, results, and bids (unchanged from before).
-- Users must explicitly hard delete auctions before they can hard delete
-- spaces that were part of those auctions.

-- Add soft delete columns
ALTER TABLE users ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE sites ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE spaces ADD COLUMN deleted_at TIMESTAMPTZ;

-- Create indexes for efficient filtering of non-deleted records
CREATE INDEX idx_users_deleted_at ON users (deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_sites_deleted_at ON sites (deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_spaces_deleted_at ON spaces (deleted_at) WHERE deleted_at IS NULL;

-- Change space_id foreign key constraints from CASCADE to RESTRICT
-- for tables that are part of auction history.
--
-- Note: PostgreSQL requires dropping and recreating constraints to change
-- ON DELETE behavior.

-- round_space_results.space_id: CASCADE -> RESTRICT
ALTER TABLE round_space_results
    DROP CONSTRAINT round_space_results_space_id_fkey;
ALTER TABLE round_space_results
    ADD CONSTRAINT round_space_results_space_id_fkey
    FOREIGN KEY (space_id) REFERENCES spaces (id) ON DELETE RESTRICT;

-- bids.space_id: CASCADE -> RESTRICT
ALTER TABLE bids
    DROP CONSTRAINT bids_space_id_fkey;
ALTER TABLE bids
    ADD CONSTRAINT bids_space_id_fkey
    FOREIGN KEY (space_id) REFERENCES spaces (id) ON DELETE RESTRICT;

-- user_eligibilities.user_id: CASCADE -> RESTRICT
-- (part of auction history, should prevent user hard delete)
ALTER TABLE user_eligibilities
    DROP CONSTRAINT user_eligibilities_user_id_fkey;
ALTER TABLE user_eligibilities
    ADD CONSTRAINT user_eligibilities_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE RESTRICT;
