-- Soft Deletes for Auction History Preservation
--
-- This migration introduces soft deletes for users, sites, and spaces to
-- support preserving auction history while allowing entities to be "deleted"
-- from the user's perspective.
--
-- ## Design Rationale
--
-- ### Users
-- When a user deletes their account:
-- - If they have auction history (bids, round_space_results, or
--   user_eligibilities), their PII is anonymized, `deleted_at` is set, and the
--   row is preserved to maintain referential integrity and allow distinguishing
--   between different deleted users in auction history. The `deleted_at` marker
--   prevents login. Non-historical data
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
-- - Hard delete: Cascades to auction history. The application checks for
--   auction history before allowing hard delete to preserve data integrity.
--
-- ### Foreign Key Constraints
-- All FK constraints use ON DELETE CASCADE for consistency. The application
-- layer enforces auction history preservation by checking for references
-- before allowing hard deletes. This approach:
-- - Keeps cascading behavior consistent across all tables
-- - Allows bulk operations (like community deletion) to work naturally
-- - Moves the "preserve auction history" logic to application code where
--   the check can be done atomically in the same DELETE statement

-- Add soft delete columns
ALTER TABLE users ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE sites ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE spaces ADD COLUMN deleted_at TIMESTAMPTZ;

-- Create indexes for efficient filtering of non-deleted records
CREATE INDEX idx_users_deleted_at ON users (deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_sites_deleted_at ON sites (deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_spaces_deleted_at ON spaces (deleted_at) WHERE deleted_at IS NULL;

-- Update space name uniqueness to allow reuse after soft-delete
-- Drop the existing unique constraint on spaces (site_id, name)
ALTER TABLE spaces DROP CONSTRAINT spaces_site_id_name_key;

-- Create a partial unique index that only applies to non-deleted spaces
-- This allows the same name to be reused after soft-delete (copy-on-write)
CREATE UNIQUE INDEX spaces_site_id_name_unique
ON spaces (site_id, name)
WHERE deleted_at IS NULL;
