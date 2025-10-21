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
