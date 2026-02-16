# Currency Modes

When you create a community, you must choose a currency mode. **This cannot be
changed later**, so it's important to understand the options before creating
your community.

The currency mode determines how auction payments work: who pays, who receives,
and whether real money is involved.

## Choosing a Mode

| Mode | Best for | Real money? |
|------|----------|-------------|
| **Points Allowance** | Internal allocation without money | No |
| **Distributed Clearing** | True common ownership with trusted members | Optional |
| **Deferred Payment** | Treasury-controlled funds | Optional |
| **Prepaid Credits** | Untrusted members or commercial use | Yes |

## Points Allowance

Members receive a recurring allowance of points from the treasury. Points are
spent in auctions and return to the treasury. No real money changes hands.

**How it works:**
- Treasury issues points to members on a schedule (e.g., 100 points/week)
- Members bid with their points in auctions
- Winning bids return points to the treasury
- Points can be saved up or transferred between members

**Use when:**
- You want fair allocation without involving money
- Auctions happen on a regular schedule (so points maintain value)
- Members have equal standing and should have equal access over time

**Example:** A student organization allocating practice room time. Each member
gets 50 points per week. Popular time slots cost more points, but everyone has
equal points to spend.

**Note:** Points Allowance works best with regular, recurring auctions. If
auctions are infrequent or one-off, points may lose their meaning after the
auction ends. For infrequent auctions, consider Distributed Clearing instead.

## Distributed Clearing

Auction winners owe IOUs split equally among all active community members,
including themselves. This achieves true common ownership: everyone shares
equally in the resource value, whether they use it or not.

**How it works:**
- Auction winners create IOUs split equally among all active members
- Winners who are active members receive their own share back
- IOUs can be settled externally (with real money) or left as internal credits
- Members can transfer credits to each other
- Mutual debts cancel out

**Use when:**
- Members trust each other to settle debts
- You want true rent redistribution (Georgist common ownership)
- Auctions may be infrequent or one-off (IOUs can be settled afterward)

**Example:** A housing cooperative with 5 active members allocating parking
spots. When Alice wins a spot for $50/month, the payment is split 5 ways: she
pays $10 each to the other 4 members, and keeps $10 as her own share of the
resource value. Net cost to Alice: $40. The other members each receive $10 for
giving up their claim to the spot.

## Deferred Payment

Auction winners owe IOUs to the community treasury rather than to individual
members. The community decides how to use the funds.

**How it works:**
- Winning bids create IOUs payable to the treasury
- Treasury can use funds for community purposes
- Settlement happens outside TinyLVT

**Use when:**
- Members are trusted to honor IOUs
- The community needs funds for maintenance, rent, or improvements
- You want auction revenue directed to a specific purpose
- Leadership controls how funds are spent

**Example:** A company using auctions to allocate desks in their office.
Employees are trusted to settle their IOUs through payroll deduction, and the
revenue offsets the office lease.

## Prepaid Credits

Members purchase credits from the treasury before they can bid. This is the
most restrictive mode, suitable when members aren't trusted to honor IOUs.

**How it works:**
- Members buy credits from treasury (payment handled outside TinyLVT)
- Only credited funds can be used for bidding
- Winning bids transfer credits back to treasury

**Use when:**
- Members may not honor debts
- You're running a commercial operation
- You need guaranteed payment before resource access

**Example:** A marina allocating boat slips. Slip holders prepay for credits
and bid for their preferred locations. No credit risk for the marina.

## Common Ownership vs. Landlord Model

The currency modes exist on a spectrum:

**Most equal (common ownership):**
- *Points Allowance* — Equal points means equal access over time, no money
  involved.
- *Distributed Clearing* — Auction proceeds go directly to members. Everyone
  shares equally in the resource value, whether they use it or not.

**Medium equal (leadership-controlled funds)**
- *Deferred Payment* / *Prepaid Credits* - Treasury funds managed by the
  community leadership. Member benefit depends on how the funds are spent.

**Least equal (landlord model):**
- *Deferred Payment* / *Prepaid Credits* — All proceeds go to one person (the
  "landlord"). Useful when someone owns the resource and wants market-based
  pricing.

Communities pick the model that matches their goals. A housing co-op might use
Distributed Clearing for true shared ownership. A commercial coworking space
might use Prepaid Credits with revenue going to cover operating costs.

## Credit Limits

For modes with IOUs (Distributed Clearing, Deferred Payment), you can set
credit limits to cap how much debt members can accumulate. This prevents
runaway obligations while still allowing flexibility.

---

*Once you've chosen your currency mode, proceed to
[Community Setup](/docs/setup).*
