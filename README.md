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

A central problem in land value taxation is the valuation of rents, so they can be charged to resource possessors.

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

### Dependencies

- The same as those used in zero2prod, where possible, though perhaps with SeaORM
- [`rust_decimal`](https://crates.io/crates/rust-decimal) for currency

### Database Schema

```
# a sybil-resistant community group for rent redistribution
communities table (
    id
    name  # name of the community
)

users table (
    id  # immutable user_id
    email  # for login
    password_hash  # for login
    display_name  # visible to others
)

# the valid community members
community_members table (
    id
    community_id  # references the communities table
    user_id
    verified
    role?  # e.g. "admin", that can add or remove other community members
)

# a divisible location containing indivisible spaces for possession that are all
# auctioned together. this defines what is covered under a single ascending
# auction.
# e.g. a room containing seats, or a building containing rooms
# (name, community_id) are unique
sites table (
    id
    name
    community_id  # sites are managed by a community, and site
                  # rents are redistributed to this community
    auction_params_id  # default auction parameters
)

# an indivisible space available for possession.
# e.g. one seat or two seats in a bundle
spaces table (
    id
    site_id
    name
    eligibility_points  # used for activity rule
    active  # whether this space is available for auction, since
            # bundling and spaces may change
)

# images of the site layout, can have multiple per site
sitemaps table (
    id
    site_id
    image_name
    sitemap_image
)

# table of auction parameters
auction_params table (
    id
    round_duration  # length of each round
    bid_increment  # encoding TBD, e.g. a,b,c,d coefficients of
                   # `a + b*round_num + c*round_num^2 + current_minimum^d`
    activity_rule_params  # encoding TBD, e.g. 50% eligibility threshold after
                          # 10 rounds, 80% after another 10 rounds, etc
)

# a complete auction instance, including rounds and results, for a specific site
auctions table (
    id
    site_id
    start_timestamp  # start time of the auction
    end_timestamp?  # end time of the auction, once concluded
    auction_params_id # parameters for this particular auction instance
)

auction_rounds table (
    id
    auction_id
    round_num
    start_timestamp
    end_timestamp  # the start time + the duration of this round
    elegibility_threshold  # fraction of a bidder's eligibility that must be
                           # met, e.g. 80%
)

space_rounds table (
    space_id, round_id  # indexed by this pair
    minimum_bid  # minimum value to obtain possession, after bid increment;
                 # starts at 0 on round 1
    winner_user_id?  # user id of the person obtaining possession at this point;
                     # can be None. if there's multiple bids, one is picked at
                     # random
)

# all bids that reach the minimum bid value for this space-round, and the user
# has eligibility. the minimum bid is the bid value
bids table (
    id
    space_id, round_id
    user_id
)

# for automated bidding if demand functions are simple.
# e.g. just wants any seat for x price, or with specification of each seat's
# value
strategies table (
    id
    auction_id
    user_id
    strategy  # how to bid during this auction; encoding TBD
)

bidder_eligibility table (
    id
    round_id
    user_id
    eligibility_points  # number of points the bidder has for this round. if
                        # threshold is not reached, next round's points are
                        # reduced. # not present on the first round; the second
                        # rounds eligibility is based on the first round's
                        # activity
)
```

## Notes

[^spectrum_auction_wikipedia]: https://en.wikipedia.org/wiki/Spectrum_auction#Auction_structure
[^combinatorial_auctions]: https://timroughgarden.org/f13/l/l8.pdf. Tim Roughgarden's course notes for [Algorithmic Game Theory](https://timroughgarden.org/f13/f13.html) and [Frontiers in Mechanism Design](https://timroughgarden.org/w14/w14.html) are excellent resources.
[^sybil_wikipedia]: https://en.wikipedia.org/wiki/Sybil_attack
