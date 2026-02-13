# Auctions

TinyLVT uses **Simultaneous Ascending Auctions** (SAAs) to allocate spaces.
This format is the gold standard in mechanism design, used for high-stakes
allocations like wireless spectrum licenses.

## How It Works

1. **All spaces are auctioned together** — Prices for all spaces start at zero
   and rise in parallel
2. **Prices increase in rounds** — Each round, minimum bids increase by a fixed
   increment
3. **Bidders can shift demand** — As prices rise, bidders move to less
   competitive spaces
4. **Auction ends when activity stops** — When no new bids are placed, the
   current high bidders win

This process reveals true demand and achieves efficient allocation: spaces go
to those who value them most, and winners pay only what's needed to outbid
others.

## Proxy Bidding

Most bidders have simple preferences: "I want one space, here's what I'd pay
for each option." Rather than watching the auction in real-time, you can set
up **proxy bidding**:

1. Enter your maximum value for each space you'd accept
2. Enable proxy bidding for the auction
3. The system bids automatically, always choosing the space where your
   surplus (value minus price) is highest

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
- Early rounds have lower thresholds; later rounds require full participation

This ensures everyone reveals their demand gradually, allowing the market to
find efficient prices.

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
in the community ledger.

---

*Learn about payment options in [Currency Modes](/docs/currency).*
