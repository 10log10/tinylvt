-- Revert soft deletes

-- Restore the original unique constraint for space names
DROP INDEX IF EXISTS spaces_site_id_name_unique;
ALTER TABLE spaces ADD CONSTRAINT spaces_site_id_name_key UNIQUE (site_id, name);

-- Drop indexes
DROP INDEX idx_spaces_deleted_at;
DROP INDEX idx_sites_deleted_at;
DROP INDEX idx_users_deleted_at;

-- Remove soft delete columns
ALTER TABLE spaces DROP COLUMN deleted_at;
ALTER TABLE sites DROP COLUMN deleted_at;
ALTER TABLE users DROP COLUMN deleted_at;
