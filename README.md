# TinyLVT

TinyLVT is an implementation of land value taxation for small-scale uses. TinyLVT guarantees useful properties:

- Spaces are allocated to the highest-value uses.
- Possessors only pay the social cost of excluding other people.
- Space is fully allocated as long as there is demand.

This pricing and allocation is achieved using repeated auctions for fixed-time possession rights. For shared ownership, rents are redistributed to community members, guaranteeing equal access to the resource and just compensation for exclusion.

For example, housemates can auction bedrooms to determine assignments and exact rent adjustments ([guide](/docs/rent-splitting.md)). Coworkers can auction desks to determine who gets the window, entryway, quiet corner, etc ([guide](/docs/desk-allocation.md)).

## Valuation by Auction

The core of a land value tax is some mechanism that determines value.

Traditional property valuation for tax purposes uses real estate transaction data to predict property prices based on the similarities between properties to be valued and properties that have sold recently. The most useful sales are between unrelated parties, called "arm's length" sales, since they show how a property would sell on the open market. Sales between related individuals, or sales with other goods involved in the transaction, might not indicate the open market value.

Arm's length sales are a kind of auction. The seller wants to obtain as high a price for their real estate as possible, and bidders compete to give the highest offer. When there are multiple interested buyers, offers may increase until there is only one buyer left.

The auction has two effects: it values the item near its opportunity cost (what someone else would have paid for it), and it allocates possession to the highest-value bidder. Valuation and allocation are inseparable. If bidders did not have the opportunity to gain possession of an item, they would have no reason to participate in the auction.

Traditional property valuation simply takes these auction results and interpolates over time, space, and property characteristics to predict how any property would sell. This can be difficult to do accurately, since no two properties or buyers are precisely the same and there may be insufficient transaction data to draw clear conclusions.

Instead of taking a small amount of valuation data and mapping it to many items, a more robust valuation method would simply auction all items at a regular interval. An item never has to have its value estimated by dissimilar items if all items are themselves precisely valued. This removes much of the estimation from the valuation process.

With regular auctions, there is no distinction made between improvements and land, and this system is best suited for cases where there are no user-created permanent improvements. Instead, the community pays for and owns any improvements that are truly permanent. This is typically the case for the small shared spaces that TinyLVT is built for. At larger scales, it also possible for people to use movable improvements to maintain ownership of them across auctions.

### Format

The auction format used is a Simultaneous Ascending Auction, which is commonly used for allocating wireless spectrum licenses.[^spectrum_auction_wikipedia][^combinatorial_auctions] All items are available for bidding in successive rounds, where prices increase incrementally each round. Bidders gradually reveal their demand and shift their bids to achieve an allocation that avoids unnecessary competition. Until someone is out-bid, they remain the holder of the item and are obligated to pay for it if the auction concludes.

The auction achieves welfare maximization (the sum of bidders' utilities) in a computationally-efficient manner, as long as bidders have non-complementary demand functions. If bidders have complementary demand, e.g. they want item A and item B together but not one item on its own, then there is combinatorial complexity that cannot be efficiently handled at scale by any auction format. Complementary demand is addressed by bundling items together, allowing bidders to win multiple items simultaneously.

For example, two people want to work together in a coworking space, and want to bid for seats that are adjacent. If all seats are individually bundled, they risk over-paying for a pair of adjacent seats. In one round they may become the high bidders for a pair of seats, but in a subsequent round they may see competition from other bidders for one of those seats. Without a new high bid on the other seat, the two people cannot relinquish their bid and shift their demand to a different location. If they shift their demand anyways, they may win an extra seat they didn't want. This is called the exposure problem.[^combinatorial_auctions] To avoid this issue, they should instead bid for pairs of seats that are bundled together in a package.

Past auction data can be an indicator of how bundles should be formed. If the price for a pair of bundled seats consistently exceeds twice the price for an individually-bundled seat, then individual seats are be converted to pairs of seats until prices equalize. The bundles are always chosen to maximize the utility of the resource.

### Activity Rule

For the auction to proceed quickly, bidders must incrementally reveal their demand all together. Demand revelation allows bidders to shift to less competitive items and achieve efficient allocation. However, bidders may be tempted to withhold their demand until prices have already stabilized, causing demand shifting to restart, and delaying the conclusion of the auction. To prevent bidders from waiting until the last minute to bid, the activity rule forces participation throughout the auction.

Each item is assigned a number of points, and bidders may only bid for as many points as they are eligible to bid for. In early rounds, bidders need to meet a reasonable fraction of their eligibility. For example, a bidder might start with 100 points of eligibility, and if the minimum threshold for maintaining eligibility is 50%, then they must bid for at least 50 points worth of items each round. If they fail to maintain their eligibility, it is decreased for the remainder of the auction.

Activity rules can limit legitimate demand shifting. If items are substitutes, they should generally be assigned the name number of eligibility points.

### Proxy Bidding

Participating in multiple rounds can be a burden for bidders. However, in most cases bidders' demand functions are simple enough to allow for proxy bidding, enabling them to define the maximum they'd pay for each item and let the system place bids for them.

In TinyLVT, the proxy bidding system maximizes surplus (the user's value minus the current price), subject to a maximum item number constraint.

## Rent Redistribution

In certain currency modes, auction proceeds are redistributed to active members. "Active" status for a member means they are eligible to receive an equal share of auction proceeds. This implements common ownership of the resource value, even if possession is unequal.

Members do not need to be active in order to bid in auctions. This is so that members can participate in auctions that have large lead times, and where the auction takes place before the member is present/active in the community. This allows communities to define activity based on properties like physical presence within the community, while allowing people to reserve space before they arrive.

## Implementation

Based on the [Zero to Production](https://github.com/LukeMathWalker/zero-to-production) reference text.

### User roles

- Member
    - can participate in auctions
    - can receive distributions if active
- Moderator
    - can also add/remove members and set their active/inactive status
- Coleader
    - can edit auction params, sites, spaces
    - can promote/demote moderators
    - can promote new coleaders
    - can set active/inactive status for all members, including other coleaders and the leader
- Leader
    - only one leader
    - can promote/demote coleaders and set their active/inactive status
    - can self demote to coleader and give leadership to a coleader

Coleaders are essentially equal leaders of the commmunity, with all priviledges except that of the leader's ability to demote coleaders. The leader is thus the final arbiter of disputes.

### Payments

In trustful communities, the currency consists of simple IOUs. By winning a space in an auction for a nonzero value, users create a debt obligation to all other active members. Debuts are settled outside of TinyLVT, and payments are recorded to cancel out the debts.

In trustless communities, users pre-fill their community balance, and can only bid up to their available funds. Funds are issued by the community treasury, controlled by community leadership. In this currency mode, auction payments return to the treasury and are not redistributed.

## Notes

[^spectrum_auction_wikipedia]: https://en.wikipedia.org/wiki/Spectrum_auction#Auction_structure
[^combinatorial_auctions]: https://timroughgarden.org/f13/l/l8.pdf. Tim Roughgarden's course notes for [Algorithmic Game Theory](https://timroughgarden.org/f13/f13.html) and [Frontiers in Mechanism Design](https://timroughgarden.org/w14/w14.html) are excellent resources.
[^sybil_wikipedia]: https://en.wikipedia.org/wiki/Sybil_attack
