# Card Payments

Communities using the Backed Credits currency mode can connect a Stripe
account to let members bid with their cards. Bids beyond a member's credit
balance place holds on their card, and the card is only charged if they win.
This page explains how card-backed bidding works for members, and how
community leaders set it up and operate it.

## How Card-Backed Bidding Works

Every bid in a Backed Credits community must be backed by real funds: your
credit balance in the community, a hold on your card, or a combination of
the two. When you bid beyond your balance, TinyLVT places a card
authorization (a hold) covering the shortfall before the bid is accepted.

A hold is not a charge. Your bank approves and reserves the amount when the
hold is placed, so card problems surface while you're bidding and can be
fixed, not after you've won. If you win, the hold is captured for the part
your balance doesn't cover. If you lose, the hold is released and you pay
nothing. Released holds carry no fees.

The auction page's Bid Funding section shows exactly where you stand:

- **Committed by bids** — what your standing and pending bids oblige you to
  pay in this auction
- **Balance backing** — the portion backed by your credit balance
- **Card hold** — the amount currently held on your card
- **Headroom** — how much further you can bid without new backing

## Setting Up Your Card

Save a card from your profile page under Payment Method. Card entry happens
on Stripe's checkout page; TinyLVT never sees your card number. You have one
saved card, shared across all your communities, and you can replace or
remove it at any time.

A saved card alone doesn't let anyone charge you. Each community needs your
explicit permission to place holds on your card, granted with the "Allow
card holds" button on the auction page (also available in the proxy bidding
section). Without the grant, your bids in that community are limited to your
credit balance. You can revoke the grant at any time; revoking stops new
holds, but holds already backing your bids stay in place until the auction
settles.

You can also bid with a card without saving anything. The auction page
offers a one-time card authorization through Stripe checkout: choose the
most you'd spend in the auction, enter your card on Stripe's page, and the
hold caps your bidding. No grant is needed for this, since you initiate the
payment yourself.

## Hold Sizing

The Authorization sizing setting on your profile controls how holds are
sized:

- **Budget holds** (the default) place one hold sized to your auction
  budget: the sum of your largest space values, up to your proxy's maximum
  spaces to win, minus your balance. One entry on your statement, no
  mid-auction card activity.
- **Minimum start** begins with a minimal hold (the currency's minimum
  charge) that grows as your bids need it. Holds stay small when bidding
  stays low, but each increase replaces the hold on your statement.

Budget holds don't account for bidder caps, so in a capped auction the
hold can be larger than your bids could ever need. Use minimum start if
you'd rather the hold track actual bidding.

When a hold needs to grow, TinyLVT places the new, larger hold first and
then cancels the old one, so your bids are never left unbacked. Both holds
may appear as pending entries on your statement until your bank processes
the cancellation, which can take hours to a few business days.

You can also size holds yourself with the "Place hold now" button, or
through the one-time checkout form. Reductions are allowed as long as the
hold still covers your committed bids.

## When Holds Are Placed

Card networks limit how long an authorization can be held, so holds are
placed no earlier than 24 hours before the auction starts. If you use proxy
bidding with a saved card and grant, TinyLVT automatically places your hold
about 24 hours before a scheduled start, giving you time to respond if the
authorization is declined or your bank requires verification. Manual
authorization opens at the same 24-hour mark; earlier attempts are rejected
with the time you can authorize from. Auctions without a scheduled start
allow authorization at any time.

If an auction's start is postponed far enough that a hold can no longer
cover it, the hold is released. Automatic holds are silently re-placed for
the new date; holds you placed yourself notify you once so you can authorize
again when the start is announced.

## If Your Card Is Declined

A declined hold never affects bids you've already placed: standing bids stay
fully backed by the existing authorization. What a decline does is pause
automatic card holds, so your proxy keeps bidding within your available
balance only, and you're notified.

To resume card-backed bidding, authorize again from the auction page. The
one-time checkout form is always offered as the recovery path; if your bank
requires verification (3D Secure), Stripe runs it on the checkout page.

One decline cause worth knowing about: each community is a separate merchant
on the card networks. Merchant-locked virtual cards (some privacy card
services lock a card to the first merchant that charges it) will decline
holds from a second community. Unlock the card or save a different one.

## Settlement and Your Statement

When the auction concludes, winners' balances are debited for their winning
totals, and the part a winner's balance can't cover is captured from their
card hold, usually within minutes. Everyone else's holds are released. The
auction page shows the outcome, card payments send an email receipt, and the
payment appears in the community transaction list as a Card Payment.

On your statement, holds appear as pending transactions under the
community's name. A captured hold converts to a posted charge; a released
hold disappears once your bank processes the reversal, which can take hours
to a few business days. If you see a pending entry after losing an auction,
it's a releasing hold, not a charge.

If a capture fails (for example the hold was canceled outside TinyLVT), the
amount remains due as a negative balance in the community. You can settle it
with a one-time card payment from the community currency page, or arrange
payment with the community's leaders.

## Credits Without a Card

The credit balance side of Backed Credits works without any card. Community
leaders can grant credits directly, recording cash paid in person, work
performed, or any other arrangement, and chore auction wins credit your
balance. A member with no card can participate fully within their balance.

Buying credits by card through TinyLVT is planned but not yet available,
pending Stripe's approval for stored value. Settling a negative balance by
card is available today.

Credits move only between members and the treasury; member-to-member
transfers are not available in this mode. Refunds of credits are at the
community's discretion, and card processing fees may be deducted from card
refunds.

## For Community Leaders

The rest of this page covers setup and operations for coleaders and leaders.

### Enabling Card Payments

Card payments require the Backed Credits currency mode, chosen at community
creation and immutable afterward. The community's currency is a real
currency denomination (USD, EUR, or GBP), also selected at creation and
immutable afterward.

Connect a Stripe account from the community settings page under Card
Payments (coleader and above). The button redirects to Stripe-hosted
onboarding, where Stripe collects the community's business and bank details.
Your community becomes the merchant of record with its own full Stripe
dashboard: charges carry your statement descriptor, you pay Stripe's
standard processing fees, and disputes are between your community and
Stripe. TinyLVT additionally takes a 1% platform fee on card charges,
deducted on Stripe's side; members always pay face value.

The settings section shows where you stand: onboarding incomplete, charges
enabled, or disconnected (if TinyLVT's access was revoked from your Stripe
dashboard — reconnect to restore automated payments). Make sure the Stripe
account settles in the community's currency; a mismatch shows a warning.

A Backed Credits community without a Stripe connection still works: leaders
grant credits, chore auctions pay out, and bids are capped at each member's
balance. Connecting later adds card-backed bidding without changing any
rules.

### Scheduling Card-Backed Auctions

Card holds can't be held indefinitely, so auctions in this mode must finish
within 48 hours of starting. An auction still running at that deadline is
canceled: every hold is released and nobody pays. The create-auction form
shows how many rounds fit in the budget for your chosen round duration; what
actually determines round count is the bid increment relative to likely
values, so avoid increments that are tiny compared to what spaces will sell
for.

Members' holds are placed starting 24 hours before the scheduled start. The
schedule keeps 12 hours of slack: you can postpone a start by up to 12 hours
without invalidating anyone's holds. Postpone further and existing holds are
released and re-placed automatically for the new date (members who placed a
hold manually are asked to authorize again).

For an auction started on demand, give members time to set values, enable
proxy bidding, and place holds before you start it; authorization is allowed
at any time while the start is unannounced.

### Settlement and Money Flow

Settlement happens in two steps. At conclusion, the ledger immediately
debits each winner's balance and credits the treasury, exactly as in the
other currency modes. Then, usually within minutes, TinyLVT captures each
winner's card hold for the portion their balance didn't cover and credits it
back to their balance, so the card payment clears the debt. The treasury
balance in TinyLVT reflects claimed revenue from the moment of conclusion;
the actual money lands in the community's Stripe balance at capture and pays
out to your bank on your Stripe payout schedule. TinyLVT never holds the
funds.

Captures of a live hold essentially always succeed, since the bank reserved
the funds at authorization. If one does fail (typically because the hold was
canceled from the Stripe dashboard or expired), the winner's negative
balance stands as a debt to the community, and coleaders and the leader are
emailed. The member can settle the debt with a card payment from the
currency page, or you can record a cash payment as a treasury grant.

If a winner's card portion lands below the currency's minimum card charge
(50 cents for USD and EUR, 30 pence for GBP), it can't be collected by card
and is forgiven; configure reserve prices and bid increments so final prices
don't land in that range.

### Operating Notes

Your Stripe dashboard is a live control surface, and manual actions there
have consequences. Canceling a hold from the dashboard removes the backing
from a member's standing bid (you and the member are notified, and if they
win, the uncovered amount becomes their debt). Refunding a captured charge
is supported from your dashboard, but TinyLVT does not automate the ledger
side: ask the member for a member-to-treasury transfer for unspent credits,
or handle the won resource outside the app.

As merchant of record, any sales tax obligations on auction payments are
your community's responsibility.

Deleting a community or a canceled auction is blocked while card payments
are in flight; wait for outstanding holds, captures, and purchases to
finish, then retry. Bids are commitments that survive a member's departure:
a departed member's backed bids stay live until settlement, they can still
win, and owed captures proceed. Payment records are preserved even if a user
deletes their account.

---

*See [Currency Modes](/docs/currency) for how Backed Credits compares to
the other modes, and [Auctions](/docs/auctions) for how bidding works.*
