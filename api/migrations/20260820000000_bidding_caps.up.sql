-- Bidding caps and supporting features. Caps are static per-bidder,
-- per-category limits on the eligibility points a bidder may hold active in
-- an auction, enforced at bid time. They let a community run related
-- auctions where what a bidder wins in one auction bounds what they may bid
-- on in another (e.g. winning a number of fungible items, then bidding on
-- locations for exactly that many).

-- An auction's name and description distinguish auctions that share a site.
-- The name is fixed at creation: space values depend on whatever the name
-- denotes, so changing its meaning requires canceling and recreating the
-- auction. The description stays editable.
ALTER TABLE auctions ADD COLUMN name VARCHAR(255);
ALTER TABLE auctions ADD COLUMN description TEXT;

-- Space categories stratify spaces for per-bidder bidding caps. They are
-- community-scoped rather than site-scoped so that related auctions on
-- different sites share category ids (e.g. an auction of fungible
-- pseudo-spaces on one site bounding a later auction of concrete locations
-- on another).
CREATE TABLE space_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (community_id, name)
);

-- NULL = uncategorized. Deletion of a referenced category is restricted
-- (default FK behavior): a category vanishing under a capped auction would
-- silently change what bidders may bid on.
ALTER TABLE spaces ADD COLUMN category_id UUID
    REFERENCES space_categories (id);

-- Serves the category-delete in-use check and the FK RESTRICT enforcement
-- on space_categories deletes.
CREATE INDEX idx_spaces_category_id ON spaces (category_id);

-- Whether per-bidder caps gate bids (see auction_bidder_caps). Immutable
-- after creation, like the name: toggling it mid-auction would change the
-- bidding rules under bidders and their proxy plans.
ALTER TABLE auctions ADD COLUMN capped BOOLEAN NOT NULL DEFAULT FALSE;

-- Per-bidder, per-category ceilings on active eligibility points in a
-- capped auction, enforced at bid time in every round (including round 0,
-- unlike the eligibility activity rule, which starts at round 1). A missing
-- row means 0: participation in a capped auction requires a cap. The NULL
-- category row governs uncategorized spaces only; it is a bucket like any
-- other, not a wildcard or default.
CREATE TABLE auction_bidder_caps (
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    category_id UUID REFERENCES space_categories (id),
    -- Denominated in eligibility points to match the active-points sum the
    -- eligibility engine computes; with 1-point spaces caps read as item
    -- counts. Booth-style counts are integral and exact in floats. Strictly
    -- positive: a 0-points cap means the same as no row, so writers delete
    -- instead of storing 0.
    points DOUBLE PRECISION NOT NULL CHECK (points > 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE NULLS NOT DISTINCT (auction_id, user_id, category_id)
);

-- Serves the category-delete in-use check and the FK RESTRICT enforcement
-- on space_categories deletes.
CREATE INDEX idx_auction_bidder_caps_category_id
    ON auction_bidder_caps (category_id);

-- Invite provenance: members record which invite they joined through, so a
-- coleader can link "@username" back to the invite email they typed without
-- seeing the member's account email. Invites carrying provenance are closed
-- by soft delete instead of vanishing: accepting a single-use invite sets
-- deleted_at, and revocation sets it when members reference the invite
-- (hard-deleting when none do — an unused invite carries no provenance).
-- A closed invite rejects acceptance.
ALTER TABLE community_invites ADD COLUMN deleted_at TIMESTAMPTZ;

-- Email-targeted invites are single-use by definition; multi-use invites
-- are anonymous links. Creation validates this; the constraint keeps the
-- invariant. (No prior flow created email multi-use rows, but normalize
-- defensively before constraining.)
UPDATE community_invites SET single_use = TRUE
    WHERE email IS NOT NULL AND NOT single_use;
ALTER TABLE community_invites ADD CONSTRAINT email_invites_single_use
    CHECK (email IS NULL OR single_use);

-- The invite the member joined through. The plain FK (no ON DELETE action)
-- means a referenced invite cannot be deleted by anyone: provenance never
-- silently degrades, and an out-of-band delete fails loudly.
ALTER TABLE community_members ADD COLUMN invite_id UUID
    REFERENCES community_invites (id);

-- Serves the reference check on invite revocation and the FK enforcement
-- on invite deletes.
CREATE INDEX idx_community_members_invite_id
    ON community_members (invite_id);

-- Self-set URL or social handle, shown beside the username. Moderators can
-- clear an abusive one.
ALTER TABLE community_members ADD COLUMN profile_link VARCHAR(255);
