# Desk Allocation

This guide walks through using TinyLVT to allocate desks (or similar shared
workspaces) within an organization. It covers setup for administrators and
auction participation for all members.

## Overview

Organizations with limited desk space can use TinyLVT to allocate desks fairly:

1. Members receive an equal allowance of points each term
2. Desks are auctioned to the highest bidders
3. Winners pay their bid amounts, which return to the treasury
4. Non-winners keep their points for the next auction

This system allocates desks to those who value them most while giving everyone
equal access over time. Members who don't win a desk still benefit—they can
save their points for future auctions or use fewer points on less-contested
desks.

---

## For Administrators

### Creating the Community

1. Go to [tinylvt.com](/) and create an account
2. Verify your email address
3. Go to Communities and click "Create Community"
4. Enter your community name (e.g., "Economics Department")
5. Select **Points Allowance** as the currency mode
6. Configure currency settings:
   - **Currency name:** Points
   - **Symbol:** P
   - **Decimal places:** 0
   - **Allowance amount:** 100 (P100 per term)
   - **Allowance period:** 3 months (or match your term length)
7. Click "Create"

You'll be the community **Leader** with full administrative access.

### Creating the Site and Spaces

1. From your community page, click "Create Site"
2. Name the site (e.g., "Main Office" or "Graduate Student Area")
3. Click "Create Site"
4. Go to the site's Settings page and configure default auction parameters:
   - **Round duration:** 1 minute
   - **Bid increment:** P5 or P10
5. Add spaces for each desk:
   - Navigate to the site's Spaces page
   - Click "Add Space" for each desk
   - Name each space clearly (e.g., "Desk 1 - Window", "Desk 2 - Corner")
   - Add descriptions noting features (natural light, proximity to exit, etc.)

**Tip:** If some desks are significantly more desirable, consider setting higher
eligibility points on those desks to prevent last-minute switching during
auctions.

### Inviting Members

1. Go to your community's Invites page
2. Create an invite link
3. Share the link with members who should participate
4. Members create accounts and join via the link

By default, new members are automatically set to **active**, meaning they'll
receive allowances when you issue them. You can change this setting in the
community's currency configuration if needed.

### Running Auctions Each Term

Before each term:

1. **Issue allowances:** Go to Treasury, select "Issue to all active members",
   enter the allowance amount (e.g., P100), and confirm
2. **Create the auction:** Navigate to the site, click "Create Auction"
   - Set the **possession period** (term start and end dates)
   - Set the **auction start time** (give members a few days to enter values)
3. The auction runs automatically at the scheduled time

---

## For Participants

### Joining the Community

1. Open the invite link shared by your administrator
2. Create a TinyLVT account (or log in if you have one)
3. Verify your email address
4. Accept the invite to join the community

### Setting Your Values

Before the auction starts:

1. Navigate to your community, then to the site with the desks
2. Open the upcoming auction
3. For each desk you'd accept, enter your **maximum value**—the most points
   you'd pay for that desk
4. Enable **proxy bidding** with max items set to 1 (or however many desks you
   want)

**How to think about values:** Your value represents how much a desk is worth
to you in points. If you value Desk A at P60 and Desk B at P40, you're saying
you'd pay up to P60 for Desk A but would switch to Desk B if Desk A's price
rose above P20 more than Desk B's price.

### During the Auction

If you've enabled proxy bidding, the system bids automatically:

1. It always bids on the desk where your surplus (value minus price) is highest
2. As prices rise, it may switch to less competitive desks
3. When prices exceed all your values, you drop out

You don't need to watch the auction in real-time—proxy bidding handles
everything based on your entered values.

### After the Auction

1. Check the auction results page for desk assignments
2. If you won a desk, your point balance is reduced by the winning bid amount
3. If you didn't win, you keep your points for future auctions

---

## Example

An economics department has 14 desks and 20 graduate students. Each term:

1. The administrator issues P100 to each active student
2. Students enter values for desks they'd want (considering location,
   lighting, noise level, etc.)
3. The auction runs—desks go to the 14 highest-valued uses
4. Winners pay their bid amounts; non-winners keep their points

Over multiple terms, points accumulate for students who don't win desks,
giving them an advantage in future auctions. Students who consistently win
desks pay more points, balancing access over time.

---

*Learn more about [Auctions](/docs/auctions) and
[Currency Modes](/docs/currency).*
