-- Drop triggers
DROP TRIGGER IF EXISTS communities_set_updated_at ON communities;
DROP TRIGGER IF EXISTS users_set_updated_at ON users;
DROP TRIGGER IF EXISTS tokens_set_updated_at ON tokens;
DROP TRIGGER IF EXISTS community_members_set_updated_at ON community_members;
DROP TRIGGER IF EXISTS community_membership_schedule_set_updated_at ON
community_membership_schedule;
DROP TRIGGER IF EXISTS sites_set_updated_at ON sites;
DROP TRIGGER IF EXISTS spaces_set_updated_at ON spaces;
DROP TRIGGER IF EXISTS site_images_set_updated_at ON site_images;
DROP TRIGGER IF EXISTS auction_params_set_updated_at ON auction_params;
DROP TRIGGER IF EXISTS bids_set_updated_at ON bids;
DROP TRIGGER IF EXISTS user_values_set_updated_at ON user_values;
DROP TRIGGER IF EXISTS use_proxy_bidding_set_updated_at ON use_proxy_bidding;

-- Drop trigger function
DROP FUNCTION IF EXISTS set_updated_at;

-- Drop indexes
DROP INDEX IF EXISTS one_leader_per_community;
DROP INDEX IF EXISTS idx_round_space_results_space_id;
DROP INDEX IF EXISTS idx_round_space_results_round_id;
DROP INDEX IF EXISTS idx_round_space_results_round_space;
DROP INDEX IF EXISTS idx_bids_user_id;
DROP INDEX IF EXISTS idx_bids_round_id;
DROP INDEX IF EXISTS idx_bids_space_id;
DROP INDEX IF EXISTS idx_bids_round_id_user_id;
DROP INDEX IF EXISTS idx_user_values_user_id;
DROP INDEX IF EXISTS idx_use_proxy_bidding_user_id;
DROP INDEX IF EXISTS idx_use_proxy_bidding_auction_id;

-- Drop tables (reverse dependency order)
DROP TABLE IF EXISTS audit_log;
DROP TABLE IF EXISTS use_proxy_bidding;
DROP TABLE IF EXISTS user_values;
DROP TABLE IF EXISTS user_eligibilities;
DROP TABLE IF EXISTS bids;
DROP TABLE IF EXISTS round_space_results;
DROP TABLE IF EXISTS auction_rounds;
DROP TABLE IF EXISTS auctions;
DROP TABLE IF EXISTS auction_params;
DROP TABLE IF EXISTS site_images;
DROP TABLE IF EXISTS spaces;
DROP TABLE IF EXISTS sites;
DROP TABLE IF EXISTS open_hours_weekday;
DROP TABLE IF EXISTS open_hours;
DROP TABLE IF EXISTS community_membership_schedule;
DROP TABLE IF EXISTS community_members;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS communities;

-- Drop enum types
DROP TYPE IF EXISTS TOKEN_ACTION;
DROP TYPE IF EXISTS ROLE;
