# Vendor Markets

This guide walks through using TinyLVT to allocate booths for a market, fair,
or similar vendor event. It layers auctions on top of familiar posted-price
booth sales, so each vendor engages with as much or as little of the mechanism
as they like.

## Overview

The starting point looks like any vendor event: the organizer offers booths of
a few types (standard, electric, double, food stall) at posted prices, and
vendors buy guaranteed access. TinyLVT adds two auctions on top:

1. **A booth auction** allocates whatever booths remain after
   posted-price sales. When the event is oversubscribed, demand sets the
   price of the remaining booths rather than a waitlist or a first-come
   scramble.
2. **A placement auction** decides which vendor gets which specific
   location. Every vendor holds a claim to some number of booths of each
   type — bought at the posted price, won in the booth auction, or both —
   and bids for the locations they prefer, limited to what they hold.

Both auctions are optional from a vendor's perspective. A vendor who buys
a booth at the posted price and never touches an auction still has their
booth; the organizer assigns them a location from whatever the placement
auction leaves unclaimed. Vendors who want another booth, or who care
where they end up, take part in whichever auction serves them — and pay
only what competition actually requires.

All locations are placed in one simultaneous auction so vendors can react
to prices and to where competitors are landing across booth types — for
example, giving up a contested corner for a cheaper mid-row spot.

---

## For Organizers

### Creating the Community

1. Go to [tinylvt.com](/) and create an account
2. Create a community with the **Backed Credits** currency mode in your
   currency, and connect Stripe so vendors can bid backed by card holds —
   see [Card Payments](/docs/card-payments)

### Categories and Sites

1. Define one **category** per booth type (e.g., "Standard booth",
   "Electric booth") from any site's Spaces page — categories are shared
   across the community
2. Create the **placement site**: the floor plan. Add one space per
   location, named by its number on the map (e.g., "Space 12"), each
   assigned its booth type's category
3. Create the **booth site**: one space per booth still unsold when
   posted-price sales close, named with letters to avoid confusion with
   locations (e.g., "Standard booth A"), all in the booth type's
   category. Add a description noting that the specific location is
   decided in the placement auction

Leave all spaces at 1 eligibility point: each booth or location then
counts as one item.

### The Two Auctions

Create both auctions early, named after the event (e.g., "October Fair —
Booths" and "October Fair — Placement"), so vendors have time to enter
values and enable proxy bidding. Create the placement auction as
**capped**, and schedule it after the booth auction concludes.

Caps are each vendor's booth claims, and you build them in two steps:

1. **Record posted-price sales by hand.** On the placement auction's
   page, set each vendor's cap for a booth type to the number they
   bought. Do this as sales happen; a vendor with no cap cannot bid for
   locations.
2. **Carry over the booth auction's results.** After the booth auction
   concludes, apply its results to the placement auction's caps in one
   step. Carry-over is additive: a vendor who bought one booth and won a
   second ends up with a cap of 2.

### After the Placement Auction

Locations that received no bids stay unallocated. Assign them to the
vendors who sat out the placement auction, and the floor plan is
complete.

---

## For Vendors

### Guaranteed Access

Buy a booth from the organizer at the posted price, as at any event. The
organizer records your claim in TinyLVT. If you do nothing else, you'll be
assigned a location after the placement auction — no bidding required.

### Bidding for Additional Booths

If you want a booth beyond what's available at the posted price, join the booth
auction:

1. Booths of a type are identical, so set one value for the whole category —
   the most you'd pay for one booth of that type
2. Enable proxy bidding with max items set to the number of booths you'd take
   in total
3. The proxy bids on the cheapest booth of the type for you, switching as
   prices rise

Prices start at zero and rise only as far as competition from other
late-deciding vendors pushes them. The remaining booths can go for less
than the posted price when late demand is thin, or for more when the
posted-price slots sold out early and demand outstrips what's left.
Buying early locks in the posted price; deciding late means taking the
market price, whichever way it lands.

### Choosing Your Location

In the placement auction, your caps appear as bidding capacity per booth type:
how many locations of each type you can pursue at once. Enter values for the
locations you prefer — a corner spot near the entrance might be worth a premium
over a mid-row one — and let the proxy chase the best deal, or bid by hand to
react to where other vendors land.

If a location matters little to you, there's no need to participate; you'll be
assigned one of the leftover locations for each booth you hold.

---

## Example

A fair has 30 standard booths and 10 electric booths. Posted prices are $80 and
$120; vendors buy 24 standard and 6 electric booths directly. The remaining 6
standard and 4 electric booths go to the booth auction, where thinner demand
from late-deciding vendors takes standard booths to $65 — below the posted
price, since bidding starts at zero and rises only as far as competition
pushes it.

Every vendor's claims become caps in the placement auction over all 40
locations. Half the vendors skip it entirely and are assigned leftover spots.
The rest bid for the locations they care about: entrance-adjacent spots clear
at $30-40 over their booth price, most others at $0.

The organizer never had to rank vendors, run a waitlist, or referee location
requests — and vendors who wanted the simple posted-price experience got
exactly that.

---

*Learn more about [Auctions](/docs/auctions) and
[Card Payments](/docs/card-payments).*
