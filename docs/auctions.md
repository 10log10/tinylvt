# Auctions

TinyLVT uses **Simultaneous Ascending Auctions** (SAAs) to allocate spaces.
This format is the gold standard in mechanism design, used for high-stakes
allocations like wireless spectrum licenses.

## How It Works

1. **All spaces are auctioned together** — Prices for all spaces start at each
   space's reserve price and rise in parallel
2. **Prices increase in rounds** — Each round, minimum bids increase by a fixed
   increment
3. **Bidders can shift demand** — As prices rise, bidders move to less
   competitive spaces
4. **Auction ends when activity stops** — When no new bids are placed, the
   current high bidders win

This process reveals true demand and achieves efficient allocation: spaces go
to those who value them most, and winners pay only what's needed to outbid
others.

## Reserve Prices

Each space has a **reserve price**: the price the first bid is placed at. By
default this is zero, but it can be set to any value, positive or negative.

**Positive reserves** are useful when a space has value even when unallocated.
For example, a shared living room normally stays common, but a member can hold
an event there if they're willing to pay enough to compensate the community
for losing it. A reserve of $50 means no one takes the space unless someone
values it at more than $50.

**Negative reserves** flip the auction into a *chore auction*: the winner is
compensated rather than charged. Bidding starts at a large negative number
(the maximum compensation the community is willing to offer) and rises toward
zero as bidders compete to accept less compensation. The space goes to
whoever is willing to do it for the least.

**Example:** Doing dishes for the week has a reserve of -$50. Two members
would accept the chore: Alice for $30 compensation, Bob for $20.
- Bidding opens at -$50 (Alice and Bob would both happily take that)
- Price rises round by round: -$45, -$40, -$35...
- At about -$30, Alice drops out (any less compensation isn't worth it to her)
- Bob wins at roughly -$30, receiving $30 in compensation

The result mirrors a normal auction: the winner pays (or receives) about what
the *second*-highest valuation is willing to accept, not their own walk-away
price.

## Proxy Bidding

Most bidders have simple preferences: "I want one space, here's what I'd pay
for each option." Rather than watching the auction in real-time, you can set
up **proxy bidding**:

1. Enter your maximum value for each space you'd accept
2. Enable proxy bidding for the auction
3. The system bids automatically, always choosing the space where your
   surplus (value minus price) is highest

**Values for chores are negative.** If you'd take a chore for $30 of
compensation, enter -$30 as your value. Proxy bidding then chases the space
where price is most below your value (i.e. where the compensation still
exceeds what you'd accept).

**Max items** controls how many spaces your proxy will win for you. For
resource auctions, this is typically 1 — you rarely need two desks or two
rooms. Chores aren't mutually exclusive in the same way: if you're willing
to do three chores for the right compensation, set max items to 3 and the
proxy will pursue the three with the most surplus relative to your values.

**Example:** You value Desk A at $80 and Desk B at $60. Prices start at $0.
- Round 1: Proxy bids on Desk A (surplus: $80 vs $60)
- Prices rise to $30: Still bids on Desk A (surplus: $50 vs $30)
- Price on Desk A rises to $60: Proxy switches to Desk B (surplus: $20 vs $30)
- Prices rise above $80 and $60: You drop out (no positive surplus remaining)

## Activity Rules

To prevent last-minute bidding that disrupts price discovery, auctions enforce
**activity rules**. You must actively participate throughout:

- Each space has eligibility points
- You must bid on enough points each round to maintain eligibility
- If you stop bidding, your eligibility decreases permanently

**Default configuration:** Set all spaces to 1 eligibility point and require
100% eligibility from round 0. This ensures bidders participate every round
while allowing them to freely move between spaces as prices rise — ideal when
spaces are substitutes (e.g., different desks in an office).

**When to use varied eligibility points:** If your spaces fall into distinct
categories with very different values (e.g., large vs. small rooms), you may
want higher eligibility points on premium spaces and a graduated threshold that
starts below 100%. This prevents bidders from "parking" on cheap items early
and then jumping to expensive ones late in the auction. However, this also
restricts legitimate flexibility, so only use it when category-switching would
genuinely disrupt price discovery.

## The Exposure Problem

Sometimes you want multiple spaces together (e.g., adjacent desks for
collaborators). Bidding on items separately creates risk: you might win one
but not the other, leaving you with something you didn't want.

**Solution: Bundles.** Spaces can be grouped into bundles that are bid on as a
unit. If you need two adjacent desks, bid on a two-desk bundle rather than
individual desks.

## Auction Parameters

When creating an auction, you'll configure:

- **Round duration** — How long each bidding round lasts
- **Bid increment** — How much prices rise each round
- **Activity thresholds** — How much bidding is required to maintain
  eligibility

## After the Auction

When the auction concludes:

1. **Winners are determined** — Highest bidders on each space
2. **Payments are calculated** — Based on winning bid amounts
3. **Currency transfers occur** — According to your community's currency mode
4. **Possession begins** — At the scheduled start time

Winners can see their allocations immediately. Payment obligations are recorded
in the community ledger. For chore auctions (negative winning prices), the
flow reverses: the winner is paid rather than charged. Who pays the
compensation depends on the currency mode — see
[Currency Modes](/docs/currency) for details.

---

*Learn about payment options in [Currency Modes](/docs/currency).*
