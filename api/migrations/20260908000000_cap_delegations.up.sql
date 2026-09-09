-- Cap delegations: members hand cap points to another member so a group can
-- bid through one bidder (e.g. three people pooling for a shared office).
-- Rows are promises, not transfers: a delegation only has effect to the
-- extent the delegator's own cap row backs it. Backing is resolved at read
-- time in creation order per (delegator, category), so an over-promised cap
-- honors the earliest delegations first. Effective cap = assigned cap -
-- backed points out + backed points in.
--
-- Only directly assigned cap backs a delegation, never received cap, so
-- delegations do not chain. Delegator-owned: only the delegator writes;
-- coleaders only read (they override via the cap itself). Frozen once the
-- auction starts.
CREATE TABLE auction_cap_delegations (
    auction_id UUID NOT NULL REFERENCES auctions (id) ON DELETE CASCADE,
    from_user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    to_user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    category_id UUID REFERENCES space_categories (id),
    -- Strictly positive like caps: 0 deletes the row.
    points DOUBLE PRECISION NOT NULL CHECK (points > 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE NULLS NOT DISTINCT (
        auction_id, from_user_id, to_user_id, category_id
    ),
    CHECK (from_user_id <> to_user_id)
);

-- Serves the category-delete in-use check and the FK RESTRICT enforcement
-- on space_categories deletes.
CREATE INDEX idx_auction_cap_delegations_category_id
    ON auction_cap_delegations (category_id);
