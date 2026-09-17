# Community Setup

This guide walks through creating and configuring a TinyLVT community.

## Before You Start

**Choose your currency mode first.** This cannot be changed after community
creation. See [Currency Modes](/docs/currency) to understand your options.

If you choose Backed Credits, you also select the currency's denomination
(USD, EUR, or GBP) at creation, and it too cannot be changed later. See
[Card Payments](/docs/card-payments) for connecting a Stripe account and
enabling card-backed bidding.

## Creating a Community

1. Log in to TinyLVT
2. Go to Communities and click "Create Community"
3. Enter your community name
4. Select your currency mode and configure its settings
5. Click "Create"

You'll be the community **Leader** with full administrative access.

## Community Structure

Communities are organized as follows:

**Community** — The group of people sharing resources. Has members, roles, and
currency settings.

**Site** — A location or logical grouping of spaces. A site defines a
collection of spaces that are auctioned together. Examples: "Main Office",
"Parking Garage", "Practice Rooms".

**Space** — An individual unit within a site that can be possessed by one
member at a time. Examples: "Desk 1", "Spot A-15", "Room 101".

**Auction** — A time-bounded event associated with a site, where all of that
site's spaces are allocated for a possession period.

**Category** — An optional label grouping similar spaces (e.g., "Standard
booth"). Categories belong to the community and are shared across its
sites. They enable bidder caps and one-step value entry — see
[Auctions](/docs/auctions).

## Member Roles

| Role | Can do |
|------|--------|
| **Member** | Participate in auctions, receive distributions |
| **Moderator** | + Add/remove members, set active status, see invite provenance, clear profile links |
| **Coleader** | + Full community management (sites, spaces, auctions, moderators) |
| **Leader** | + Manage coleaders, transfer leadership |

Coleaders have full administrative control over the community. The Leader role
exists to provide final authority: only the Leader can promote or demote
Coleaders, and only the Leader can transfer leadership to someone else.

## Inviting Members

1. Go to your community's Invites page
2. Create an invite — either an open invite link anyone can use, or an
   invite addressed to a specific email
3. Share the link with people you want to join
4. They'll create an account (if needed) and join your community

Open invite links can be single-use or multi-use. Email-addressed invites
are always single-use.

Invites keep a record of who joined through them. Once a single-use invite
is accepted, or an invite that members joined through is revoked, it stays
in the issued list as a closed, read-only record. In the member list,
moderators see which invite email each member joined through ("Invited:
jane@example.com"), which connects self-chosen usernames to the people you
invited without exposing anyone's account email. Revoking an invite nobody
used simply removes it.

## Member Profiles

Members can add a **profile link** to their entry in the member list — a
website or social handle shown under their username, so others can see who
they are. Set yours from your own row's menu in the member list.
Moderators can clear an inappropriate link.

## Active vs Inactive Members

**Active members** can bid in auctions. In Distributed Clearing mode they
also receive their share of auction proceeds, and in Points Allocation mode
they receive allowances.

**Inactive members** remain in the community and keep their account, but
can't place bids and don't receive distributions. Bids they already hold
when deactivated stay in place. This is useful for:

- Members who are temporarily away
- Members who must meet a requirement before renting space, such as a
  vendor whose seller's permit needs to be on file before each market

Moderators toggle active status from the member list, and the community's
currency settings choose whether newly joined members start out active. A
community that checks requirements before allowing bids would have new
members start inactive and activate them once cleared.

## Creating Sites and Spaces

1. From your community page, click "Create Site"
2. Name the site and configure settings
3. Add spaces to the site with names and descriptions

**Tip:** Think about how spaces relate to each other. If people often want
adjacent spaces together, consider creating bundles.

### Categories

If your spaces fall into types, define categories on the site's Spaces
page and assign one to each space when creating or editing it. Categories
belong to the community, so the same set is available on every site.
They're used for bidder caps and for setting one proxy value across a
whole category — see [Auctions](/docs/auctions).

### Reserve Prices

Each space has a **reserve price** — the starting price the first bid is
placed at. The default of zero works for most cases, but you can change it
per space:

- **Positive reserve:** No one wins the space unless they value it above this
  threshold. Useful when the space has value when left common (e.g. a shared
  living room that stays common unless someone pays enough to host an event).
- **Negative reserve:** Turns the auction into a chore auction. Bidding opens
  at the negative amount and rises toward zero; the winner is compensated
  rather than charged. Set the reserve to the largest compensation the
  community is willing to offer.

See [Auctions](/docs/auctions) for how reserves shape bidding.

## Running Your First Auction

1. Navigate to a site
2. Click "Create Auction"
3. Set the auction parameters:
   - Name and description (optional; the description can be edited later,
     the name cannot)
   - Start time
   - Round duration
   - Bid increment
   - Possession period
4. Members can enter proxy bids before the auction starts
5. The auction runs automatically at the scheduled time

To limit how much each bidder can win by category, create the auction as
**capped** and assign per-bidder caps from the auction's Settings tab —
entered by hand or carried over from a concluded auction's results. See
[Auctions](/docs/auctions) for how caps work.

In Backed Credits communities with card payments enabled, auctions must
finish within 48 hours of starting, and members place card holds in the 24
hours before the start. See
[Card Payments](/docs/card-payments) for how this shapes scheduling.

---

*Learn more about [Auctions](/docs/auctions) and [Currency Modes](/docs/currency).*
