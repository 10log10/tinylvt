# TinyLVT

TinyLVT is an implementation of land value taxation for small-scale uses. Parts of a shared space can be allocated using TinyLVT to guarantee useful properties:

- Spaces are allocated to the highest-value uses.
- Possessors only pay the social cost of excluding other people.
- Space is fully allocated as long as there is demand.

This pricing and allocation is achieved using repeated auctions for fixed-time possession rights. Parameters include:

- The things available for possession
- The duration of possession
- The parameters of the auction

For example, a coworking space could have seats available for possession for 1-hour time slots, with auctions that take place 30 minutes before the start of possession.

Rents can be redistributed to community members to achieve common value ownership, guaranteeing equal access to the resource regardless of the price level.

## Valuation by Auction

The core of a land value tax is some mechanism that determines value.

Traditional property valuation for tax purposes uses real estate transaction data to predict property prices based on the similarities between properties to be valued and properties that have sold recently. The most useful sales are between unrelated parties, called "arm's length" sales, since they show how a property would sell on the open market. Sales between related individuals, or sales with other goods involved in the transaction, might not indicate the open market value.

Arm's length sales are a kind of auction. The seller wishes to obtain as high a price for their real estate as possible, and bidders compete to give the highest offer. When there are multiple interested buyers, offers may increase until there is only one buyer left who willing to pay the high price.

The auction has two effects: it values the item near its opportunity cost (what someone else would have paid for it), and it allocates possession to the highest-value bidder. Valuation and allocation are inseparable. If bidders did not have the opportunity to gain possession of an item, they would have no reason to participate in the auction.

Traditional property valuation simply takes these auction results and interpolates over time, space, and property characteristics to predict how any property would sell. This process is never without potential faults, since no two properties or buyers are precisely the same and there may be insufficient transaction data to draw clear conclusions.

Instead of taking a small amount of valuation data and mapping it to many items, a more robust valuation method would simply auction all items at a regular interval. An item never has to have its value estimated by dissimilar items if all items are themselves precisely valued. This removes much of the estimation from the valuation process.

With regular auctions, there is no distinction made between improvements and land, and this system is best suited for cases where there are no user-created permanent improvements. Instead, the community pays for and owns any improvements that are truly permanent. This is typically the case for small shared spaces like a coworking space.

### Format

The auction format used is a Simultaneous Ascending Auction, which is commonly used for allocating wireless spectrum licenses.[^spectrum_auction_wikipedia][^combinatorial_auctions] All items are available for bidding in successive rounds, where prices increase incrementally each round. Bidders gradually reveal their demand and shift their bids to achieve an allocation that avoids unnecessary competition. Until someone is out-bid, they remain the holder of the item and are obligated to pay for it if the auction concludes.

The auction achieves welfare maximization (the sum of bidders' utilities) in a computationally-efficient manner, as long as bidders have non-complementary demand functions. If bidders have complementary demand, e.g. they want item A and item B together but not one item on its own, then there is combinatorial complexity that cannot be efficiently handled at scale by any auction format. Complementary demand is addressed by bundling items together, allowing bidders to win multiple items simultaneously.

For example, two people want to work together in a coworking space, and want to bid for seats that are adjacent. If all seats are individually bundled, they risk over-paying for a pair of adjacent seats. In one round they may become the high bidders for a pair of seats, but in a subsequent round they may see competition from other bidders for one of those seats. Without a new high bid on the other seat, the two people cannot relinquish their bid and shift their demand to a different location. If they shift their demand anyways, they may win an extra seat they didn't want. This is called the exposure problem.[^combinatorial_auctions] To avoid this issue, they should instead bid for pairs of seats that are bundled together in a package.

Past auction data can be an indicator of how bundles should be formed. If the price for a pair of bundled seats consistently exceeds twice the price for an individually-bundled seat, then individual seats are be converted to pairs of seats until prices equalize. The bundles are always chosen to maximize the utility of the resource.

Auction parameters:

- The time between auction rounds
- The initial bid increment between rounds
- The rate of increase of the minimum bid increment (ensures the auction finishes quickly)
- The activity rule

### Activity Rule

For the auction to proceed quickly, bidders must incrementally reveal their demand all together. Demand revelation allows bidders to shift their demand to less competitive items and achieve efficient allocation. However, bidders may be tempted to withhold their demand until prices have already stabilized, causing demand shifting to restart, and delaying the conclusion of the auction. To prevent bidders from waiting until the last minute to bid, the activity rule forces participation throughout the auction.

Each item is assigned a number of points, and bidders may only bid for as many points as they are eligible to bid for. In early rounds, bidders need to meet a reasonable fraction of their eligibility. For example, a bidder might start with 100 points of eligibility, and if the minimum threshold for maintaining eligibility is 50%, then they must bid for at least 50 points worth of items each round. If they fail to maintain their eligibility, it is decreased for the remainder of the auction.

When activity reduces or prices stabilize, the eligibility threshold is increased, e.g. to 50%, 80%, 95% then 100%. Having slack in early rounds allows bidders to shift their demand from lower-value bundles to higher-value bundles if it makes sense for them to do so.

### Proxy Bidding

Participating in multiple rounds can be a burden for bidders. However, in most cases bidders' demand functions are simple enough to allow for proxy bidding, allowing them to define a simple bidding strategy and let the system place bids for them.

For example, if someone just wants one seat for X price or lower, then the proxy bidding can bid for whichever seat is currently priced the lowest, as long as the minimum bid is still under their price cap. This way the user does not need to actively watch the auction. They can setup their proxy bidding ahead of time and get notified of the result when the auction has concluded.

Or, if someone wants to specify individual valuations for each seat, and they only want one, then this is also a simple enough demand function to have automatic bidding. They submit their valuations, and automatically bid for whichever seat has the lowest price compared to their valuation for it, maximizing their utility.

## Rent Redistribution

Rents can be redistributed. A parameter determines the fraction of proceeds that are redistributed. At 100%, all recipients effectively become joint owners of the value of the rental value, and have equal access regardless of the price level.

Care must be taken to avoid cheating in the rent redistribution. Since rents have value, there is the incentive for people to manufacture duplicate identities.[^sybil_wikipedia] This is avoided using a community members table that defines which use ids are eligible for rent distributions.

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

### Acitive/invactive

Active members are eligible to receive an equal share of auction proceeds. A community may have zero or more active members. With zero active members, no payment is made upon auction conclusion. This is something of an edge case.

With one active member, the community acts like a single owner / multiple renter setup. All proceeds are captured by the active member. This is useful when the community is renting space from an outside entity and wants to primarily use its auction proceeds to cover that obligation. Alternatively, this can be used when the community is open to anyone to bid, and there is no defense against sybil attacks, where fake identities are used to capture an outsized share of the rent distributions.

With multiple active members, the community has a joint ownership of the resource value.

Members do not need to be active in order to bid in auctions. This is so that members can participate in auctions that have large lead times, and where the auction takes place before the member is present/active in the community. This allows communities to define activity based on properties like physical presence within the community, while allowing people to reserve space before they arrive.

### Payments

In trustful communities, the auction currency are simple IOUs. By winning a space in an auction for a nonzero value, users create a debt obligation to all other active members. Debuts are settled outside of TinyLVT, and payments are recorded to cancel out the debts. Mutal debts also cancel out.

In trustless communities, users pre-fill their TinyLVT account with funds, and can only bid up to their available funds. Upon winning an auction their bid value is deducted from their balance and transferred to the other active members in the community. [Not planned for MVP.]

Since auctions can have significant lead time (bidding happens before the period of possession), the recipients of the auction proceeds are determined when the period of possession begins. For IOUs, this means that the IOU quantity is established at the conclusion of the auction, but the recipients of those IOUs are not determined until space possession begins. For real balances, this means that the bid value is deducted from the winner's balance at the conclusion of the auction, and that the bid value is only distributed to other community members when space possession begins. (In the future, there will be support for incremental distribution throughout the possession period, parameterized by a `distribution_interval` column in the `sites` table.)

## Notes

[^spectrum_auction_wikipedia]: https://en.wikipedia.org/wiki/Spectrum_auction#Auction_structure
[^combinatorial_auctions]: https://timroughgarden.org/f13/l/l8.pdf. Tim Roughgarden's course notes for [Algorithmic Game Theory](https://timroughgarden.org/f13/f13.html) and [Frontiers in Mechanism Design](https://timroughgarden.org/w14/w14.html) are excellent resources.
[^sybil_wikipedia]: https://en.wikipedia.org/wiki/Sybil_attack
