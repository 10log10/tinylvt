-- Roles:
-- 'leader',  -- Only one leader
-- 'coleader',  -- Same privileges as leader, but can have multiple
-- 'moderator',  -- Lower-level privileges, but above member
-- 'member'  -- Default membership level
CREATE TYPE ROLE AS ENUM ('member', 'moderator', 'coleader', 'leader');

-- Token actions for email verification and password reset
CREATE TYPE TOKEN_ACTION AS ENUM ('email_verification', 'password_reset');

CREATE TABLE communities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    -- Whether new members are active (eligible for distributions) by default.
    new_members_default_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    display_name VARCHAR(255),
    -- If email ownership has been verified; part of signup flow, required to
    -- create or join communities
    email_verified BOOLEAN NOT NULL DEFAULT false,
    balance NUMERIC(20, 6) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Tokens are emailed and are specific to 'email_verification', 'password_reset'
-- actions.
CREATE TABLE tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), -- the token
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    action TOKEN_ACTION NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false, -- can only be used once
    expires_at TIMESTAMPTZ NOT NULL, -- must be used before expiry
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

CREATE TABLE community_members (
    -- Cascade: if a community is deleted, memberships are deleted too
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role ROLE NOT NULL,
    -- An inactive member is ineligible to receive distributions.
    -- Can be set automatically by community_membership_schedule if user matches
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (community_id, user_id)
);

CREATE UNIQUE INDEX one_leader_per_community
ON community_members (community_id)
WHERE role = 'leader';

CREATE TABLE community_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    -- If provided, the accepting user email must match. Otherwise it's an open
    -- invite to anyone with the invite id.
    email VARCHAR(255),
    single_use BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- A future schedule of community membership that results in automatic
-- updating of the `is_active` state.
--
-- There can be multiple entries for a given email address if membership comes
-- and goes. If a user email is not present in the schedule, activity state is
-- only manually configured.
--
-- The email field can be an ordinary string or a hex digest of the SHA256 of
-- the email. Both are checked. Hashing reduces the privacy loss of users that
-- have not yet voluntarily signed up, but that are included in a community
-- schedule.
CREATE TABLE community_membership_schedule (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    email VARCHAR(255) NOT NULL,  -- email identifier
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Auction parameters are immutable and copy-on-write if they are used in a
-- past auction.
CREATE TABLE auction_params (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Length of time of each round.
    round_duration INTERVAL NOT NULL,
    -- 20 digits total, with 6 units of precision
    bid_increment NUMERIC(20, 6) NOT NULL,
    -- Eligibility requirements as the auction progresses. Determines each
    -- round's eligibility_threshold
    activity_rule_params JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Open hours for a site when possession takes place. Can be used for holidays
-- by updating the open hours within a week of the closure.
--
-- Maps 1-1 with a site, so when the site's open hours are updated these hours
-- get updated
CREATE TABLE open_hours (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

-- If a day of the week is absent, the site is assumed to be closed that day.
CREATE TABLE open_hours_weekday (
    open_hours_id UUID NOT NULL REFERENCES open_hours (id) ON DELETE CASCADE,
    -- 1 = Monday, 7 = Sunday
    day_of_week SMALLINT NOT NULL CHECK (day_of_week BETWEEN 1 AND 7),
    open_time TIME NOT NULL,  -- Local time
    close_time TIME NOT NULL, -- Local time (if before open_time, is next day)
    PRIMARY KEY (open_hours_id, day_of_week)
);

-- Images for sites or spaces.
CREATE TABLE site_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    image_data BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    UNIQUE (community_id, name)
);

-- A location consisting of indivisible spaces available for rent, and for
-- which auctions take place.
CREATE TABLE sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    default_auction_params_id UUID NOT NULL REFERENCES auction_params (id),

    -- Auction auto scheduling parameters --

    -- Duration of possession and period between auctions.
    possession_period INTERVAL NOT NULL,
    -- Amount of time before the change in possession that the auction begins.
    auction_lead_time INTERVAL NOT NULL,
    -- Amount of time before the start of auction that the auction row exists
    -- and proxy bids can be prepared.
    proxy_bidding_lead_time INTERVAL NOT NULL,
    -- If not present, the site is assumed to be open all the time.
    open_hours_id UUID REFERENCES open_hours (id) ON DELETE SET NULL,

    -- Whether this site is automatically scheduled for auction. Otherwise
    -- auctions are manually triggered.
    auto_schedule BOOLEAN NOT NULL DEFAULT true,

    -- IANA time zone, e.g. 'America/Los_Angeles'.
    -- If not provided, datetime math uses UTC and the times render in the
    -- users's local time.
    timezone TEXT,

    -- Image is optional if the location is otherwise well-described.
    site_image_id UUID REFERENCES site_images (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    UNIQUE (community_id, name)
);

-- An individual space available for possession.
CREATE TABLE spaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    eligibility_points DOUBLE PRECISION NOT NULL,
    -- Whether this space is available for auction, which can be changed based
    -- on bundling.
    is_available BOOLEAN NOT NULL DEFAULT true,
    -- Image is optional if the location is otherwise well-described.
    site_image_id UUID REFERENCES site_images (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    UNIQUE (site_id, name),
    CHECK (eligibility_points >= 0.0)
);

CREATE TABLE auctions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id UUID NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    -- The specific possession period being auctioned.
    possession_start_at TIMESTAMPTZ NOT NULL,
    possession_end_at TIMESTAMPTZ NOT NULL,
    -- Start and end times of the auction.
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ, -- Filled in when the auction completes.
    -- The auction params used in this auction.
    auction_params_id UUID NOT NULL REFERENCES auction_params (id),
    -- Scheduler failure tracking for debugging and backoff
    scheduler_failure_count INTEGER NOT NULL DEFAULT 0,
    scheduler_last_failed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- Not directly editable by users (only read/list), since the system manages
-- the auction rounds itself.
CREATE TABLE auction_rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    -- The index of the round in the auction, starting at 0.
    round_num INTEGER NOT NULL,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    -- Fraction of the bidder's eligibility that must be met, e.g. 80%
    eligibility_threshold DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    UNIQUE (auction_id, round_num),
    -- Elibility requirements can be 0% or 100% of current user eligibility.
    -- 0% means no eligibility is required, whereas 100% prevents any demand
    -- shifting to higher-value spaces.
    CHECK (eligibility_threshold >= 0.0 AND eligibility_threshold <= 1.0)
);

-- The current winner (until the next round) of a space.
--
-- Populated when a round concludes. `value` always exists, but winning_user
-- only exists if someone has big for the space.
CREATE TABLE round_space_results (
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    winning_user_id UUID REFERENCES users (id),
    -- space value at the conclusion of this round
    value NUMERIC(20, 6) NOT NULL,
    PRIMARY KEY (space_id, round_id)
);
CREATE INDEX idx_round_space_results_space_id ON round_space_results (space_id);
CREATE INDEX idx_round_space_results_round_id ON round_space_results (round_id);
CREATE INDEX idx_round_space_results_round_space ON round_space_results (
    round_id, space_id
);

-- All bids for spaces in an auction round that meet (are) the minimum bid
-- increment.
--
-- A user must have the necessary balance to place a bid.
--
-- At the end of a round one of the valid bidders is chosen randomly as the
-- round winner.
CREATE TABLE bids (
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (space_id, round_id, user_id)
);
CREATE INDEX idx_bids_user_id ON bids (user_id);
CREATE INDEX idx_bids_round_id ON bids (round_id);
CREATE INDEX idx_bids_space_id ON bids (space_id);
CREATE INDEX idx_bids_round_id_user_id ON bids (round_id, user_id);

-- User eligibility across auction rounds.
--
-- Like round_space_results, this is updated after a round concludes, and
-- indicates how much eligibility the user has for the next round, based on the
-- previous round's eligibility threshold and bidder activity. Only exists for
-- round numbers greater than zero.
CREATE TABLE user_eligibilities (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    round_id UUID NOT NULL REFERENCES auction_rounds (id) ON DELETE CASCADE,
    eligibility DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (user_id, round_id),
    CHECK (eligibility >= 0)
);

-- User-assigned values for each space, for proxy bidding.
--
-- The proxy bidding mechanism automatically bids for the space that has the
-- largest difference between the user's value and the current value. This is a
-- simple utility-greedy strategy that ignores the value of bundles of items
-- (complementary demand).
CREATE TABLE user_values (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    space_id UUID NOT NULL REFERENCES spaces (id) ON DELETE CASCADE,
    value NUMERIC(20, 6) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (user_id, space_id),
    CHECK (value >= 0)
);
CREATE INDEX idx_user_values_user_id ON user_values (user_id);

-- A row present in this table indicates that a user's space values should be
-- used for automatic proxy bidding.
--
-- max_items defines how many items the user is willing to win. The proxy
-- bidding system will bid for up to that many items, attempting to maximize
-- the user's surplus (max_value - current_price).
CREATE TABLE use_proxy_bidding (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    max_items INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    PRIMARY KEY (user_id, auction_id)
);
CREATE INDEX idx_use_proxy_bidding_user_id ON use_proxy_bidding (user_id);
CREATE INDEX idx_use_proxy_bidding_auction_id ON use_proxy_bidding (auction_id);
CREATE INDEX idx_use_proxy_bidding_user_id_auction_id ON use_proxy_bidding
(user_id, auction_id);


-- Automatic update triggers

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = current_timestamp;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER communities_set_updated_at
BEFORE UPDATE ON communities
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER users_set_updated_at
BEFORE UPDATE ON users
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER tokens_set_updated_at
BEFORE UPDATE ON tokens
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER community_members_set_updated_at
BEFORE UPDATE ON community_members
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER community_membership_schedule_set_updated_at
BEFORE UPDATE ON community_membership_schedule
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER sites_set_updated_at
BEFORE UPDATE ON sites
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER spaces_set_updated_at
BEFORE UPDATE ON spaces
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER site_images_set_updated_at
BEFORE UPDATE ON site_images
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER auction_params_set_updated_at
BEFORE UPDATE ON auction_params
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER auctions_set_updated_at
BEFORE UPDATE ON auctions
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER auction_rounds_set_updated_at
BEFORE UPDATE ON auction_rounds
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER bids_set_updated_at
BEFORE UPDATE ON bids
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER user_values_set_updated_at
BEFORE UPDATE ON user_values
FOR EACH ROW
EXECUTE FUNCTION set_updated_at();


-- Audit log

-- Track who changed what and when.
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID, -- the user who performed the action
    action TEXT NOT NULL, -- e.g., 'update_role', 'place_bid'
    target_table TEXT,
    target_id UUID,
    details JSONB, -- anything relevant: old/new values, diffs, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);
