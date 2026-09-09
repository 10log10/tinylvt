ALTER TABLE community_members DROP COLUMN profile_link;
ALTER TABLE community_members DROP COLUMN invite_id;
ALTER TABLE community_invites DROP CONSTRAINT email_invites_single_use;
-- Closed invites only exist as provenance records; before this migration a
-- used or revoked invite was removed outright, so reopening them would let
-- the links be accepted again.
DELETE FROM community_invites WHERE deleted_at IS NOT NULL;
ALTER TABLE community_invites DROP COLUMN deleted_at;
DROP TABLE auction_bidder_caps;
ALTER TABLE auctions DROP COLUMN capped;
ALTER TABLE spaces DROP COLUMN category_id;
DROP TABLE space_categories;
ALTER TABLE auctions DROP COLUMN description;
ALTER TABLE auctions DROP COLUMN name;
