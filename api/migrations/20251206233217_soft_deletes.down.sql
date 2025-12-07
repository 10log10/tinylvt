-- Revert soft deletes and restore original CASCADE behavior

-- Restore user_eligibilities.user_id: RESTRICT -> CASCADE
ALTER TABLE user_eligibilities
    DROP CONSTRAINT user_eligibilities_user_id_fkey;
ALTER TABLE user_eligibilities
    ADD CONSTRAINT user_eligibilities_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE;

-- Restore bids.space_id: RESTRICT -> CASCADE
ALTER TABLE bids
    DROP CONSTRAINT bids_space_id_fkey;
ALTER TABLE bids
    ADD CONSTRAINT bids_space_id_fkey
    FOREIGN KEY (space_id) REFERENCES spaces (id) ON DELETE CASCADE;

-- Restore round_space_results.space_id: RESTRICT -> CASCADE
ALTER TABLE round_space_results
    DROP CONSTRAINT round_space_results_space_id_fkey;
ALTER TABLE round_space_results
    ADD CONSTRAINT round_space_results_space_id_fkey
    FOREIGN KEY (space_id) REFERENCES spaces (id) ON DELETE CASCADE;

-- Drop indexes
DROP INDEX idx_spaces_deleted_at;
DROP INDEX idx_sites_deleted_at;
DROP INDEX idx_users_deleted_at;

-- Remove soft delete columns
ALTER TABLE spaces DROP COLUMN deleted_at;
ALTER TABLE sites DROP COLUMN deleted_at;
ALTER TABLE users DROP COLUMN deleted_at;
