# Rent Splitting

This guide walks through using TinyLVT to fairly allocate rooms and determine
rent obligations among housemates. It covers setup for the person organizing
the household and participation for all housemates.

## Overview

When housemates share a rental, rooms often differ in value—some have more
space, better light, or a private bathroom. TinyLVT solves this fairly:

1. Split the base rent evenly among all housemates
2. Auction the rooms to determine their relative value
3. Redistribute the auction proceeds equally to everyone
4. Adjust each person's rent based on the room they won

Winners of nicer rooms pay more; those with less desirable rooms pay less.
Everyone shares equally in the total value of the housing.

---

## For the Organizer

### Creating the Community

1. Go to [tinylvt.com](/) and create an account
2. Verify your email address
3. Go to Communities and click "Create Community"
4. Enter your community name (e.g., "123 Main St Housemates")
5. Select **Distributed Clearing** as the currency mode
6. Configure currency settings:
   - **Currency name:** dollars (or your local currency)
   - **Symbol:** $
   - **Decimal places:** 2
   - **Balances visible to members:** Yes
   - **New members active by default:** Yes
7. Click "Create"

### Creating the Site and Spaces

1. From your community page, click "Create Site"
2. Name the site (e.g., "House Rooms")
3. Click "Create Site"
4. Go to the site's Settings page and configure auction parameters:
   - **Round duration:** 1–5 minutes
   - **Bid increment:** About 10% of the expected maximum room premium
     (e.g., $10 if the nicest room might command a $100/month premium)
5. Add spaces for each room:
   - Navigate to the site's Spaces page
   - Click "Add Space" for each room
   - Name each room clearly (e.g., "Master Bedroom", "Back Room - Small")
   - Add descriptions noting features (size, bathroom access, closet, etc.)

**Tip on eligibility points:** If rooms vary significantly in value, set
eligibility points proportional to expected value. This prevents someone from
bidding on a cheap room early, then switching to an expensive room late in
the auction.

### Inviting Housemates

1. Go to your community's Invites page
2. Create an invite link
3. Share the link with your housemates
4. Everyone creates accounts and joins via the link

### Running the Auction

1. **Create the auction:** Navigate to the site, click "Create Auction"
   - Set the **possession period** (e.g., the next year of the lease)
   - Set the **auction start time** (give everyone a few days to enter values)
   - Communicate that prices represent the premium for the full possession
     period, not monthly amounts
2. Have everyone enter their values and enable proxy bidding before the
   auction starts
3. The auction runs automatically at the scheduled time

### After the Auction

When the auction concludes:

1. Winners are assigned their rooms
2. IOUs are automatically issued—winners owe their bid amount, split equally
   among all active housemates (including themselves)
3. Each housemate's balance shows their net position:
   - **Positive balance:** Others owe you money (you pay less rent)
   - **Negative balance:** You owe others (you pay more rent)

### Settling Rent

Convert balances to monthly rent adjustments by dividing each person's balance
by the number of months in the possession period, then subtracting this
adjustment to the evenly-split base rent. See the [example below](#example) for
details.

---

## For Participants

### Joining the Community

1. Open the invite link shared by your organizer
2. Create a TinyLVT account (or log in if you have one)
3. Verify your email address
4. Accept the invite to join the community

### Setting Your Values

Before the auction starts:

1. Navigate to your community, then to the site with the rooms
2. Open the upcoming auction
3. For each room you'd accept, enter your **maximum value**—the most you'd
   pay as a premium for that room over the full possession period
4. Enable **proxy bidding** with max items set to 1

**How to think about values:** Consider what each room is worth to you in
total additional rent over the lease period. If you'd pay up to $50/month
extra for the master bedroom over a 12-month lease, your value is $600.

### During the Auction

With proxy bidding enabled, the system bids automatically:

1. It bids on the room where your surplus (value minus price) is highest
2. As prices rise, it may switch to less competitive rooms
3. When prices exceed all your values, you drop out

You don't need to watch the auction—proxy bidding handles everything.

### After the Auction

1. Check the auction results for room assignments
2. View your balance in the community's currency page:
   - **Negative balance:** You won a room and owe a premium
   - **Positive balance:** Others' premiums are redistributed to you
3. Work with your organizer to convert balances into monthly rent adjustments

---

## Example {#example}

Three housemates—Alice, Bob, and Carol—share a house with monthly rent of
$3,000. They run a 12-month auction for three rooms.

**Auction results:**
- Master bedroom: Alice wins at $1,200
- Middle room: Bob wins at $600
- Small room: Carol wins at $0

**Total proceeds:** $1,800, redistributed equally → $600 each

**Balances:**
- Alice: Paid $1,200, received $600 → Balance: -$600
- Bob: Paid $600, received $600 → Balance: $0
- Carol: Paid $0, received $600 → Balance: +$600

**Monthly rent (base $1,000 - balance / 12 months):**
- Alice: $1,000 + $50 = $1,050
- Bob: $1,000 + $0 = $1,000
- Carol: $1,000 - $50 = $950

Everyone pays according to the value of their room, and the total still
equals $3,000.

---

*Learn more about [Auctions](/docs/auctions) and
[Currency Modes](/docs/currency).*
