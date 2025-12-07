-- Revert soft deletes

-- Drop indexes
DROP INDEX idx_spaces_deleted_at;
DROP INDEX idx_sites_deleted_at;
DROP INDEX idx_users_deleted_at;

-- Remove soft delete columns
ALTER TABLE spaces DROP COLUMN deleted_at;
ALTER TABLE sites DROP COLUMN deleted_at;
ALTER TABLE users DROP COLUMN deleted_at;
