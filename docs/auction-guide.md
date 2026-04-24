<!-- @@section:intro -->

# Interactive Guide to Cooperative Auctions

Three roommates need to divide bedrooms. One room is bigger, one has a balcony, and one is next to the kitchen. They can argue about it, draw straws, or let whoever moved in first claim the best room. None of these options are satisfying. Someone always feels shortchanged.

An auction solves this problem elegantly. Each person bids for the rooms they want, the highest bid for each room wins, and the proceeds are shared equally. The person who gets the best room pays more rent, and the person who gets the worst room pays less. Nobody has to argue about what's fair, since the bids determine it exactly.

This guide walks through how these **cooperative auctions** work, starting from the basics and building up to multi-item auctions, settlement mechanics, and less-obvious applications like group decision-making. Each auction has an interactive simulation you can experiment with.

## Regular Auctions

Before we can see how cooperative auctions work, we need to cover how auctions work in general. We'll start with the simplest possible case: one item, two bidders, and no shared ownership. Pat is selling her bike and Nina and Omar both want it.

They agree to use an ascending auction to sell the bike. In ascending auctions, the price rises over multiple rounds until only one bidder is left. This format is economically efficient (the item goes to the person who values it most) and easy to interpret. The price starts at zero and each round it increases by a fixed bid increment.

The top of the simulator defines the things available for auction, the bidders who are participating, and the amount that each bidder would pay to have each item. Here, Nina is willing to pay up to $150 for the bike, and Omar is willing to pay up to $100.

Below this, the simulation is visualized in three sections. First, the price, high bid, and new bids for the selected round. The price is the current high bid, and new bids are one increment above. Second, a plot of the price over the course of the auction, with each round represented as a discrete step when a new bid is placed. Third, each bidder's activity. Dots represent bids.

<!-- @@section:after_bike_auction -->

The progression of the auction is straightforward. Nina and Omar both continue bidding for the bike until the price gets to $100. At this point, Omar would be bidding for the bike at a price of $110, which is above his value for the bike, so he drops out. The auction stops since there's no more bidding activity.

Notice how Nina wins the bike, but the price is based on Omar's value. This is a second-price mechanism, and it's an important feature of the auction. It means that Nina is not penalized for having a higher value for the bike. She only pays what the next-highest bidder would have paid for it.

To see why this matters, consider a first-price auction where Nina and Omar write down sealed bids and Pat accepts the higher one. Nina would want to bid just above Omar's value to avoid overpaying, which forces her to predict what Omar will bid. Guess too low and she loses the bike at a price she would have paid. Guess too high and she overpays. Second-price mechanisms remove this guessing. Nina's optimal strategy is simply to bid her true value, knowing her payment will only rise as high as needed to beat Omar.

Note that in a real auction, Nina's and Omar's values are private. They only find out how the other person values the bike as the price rises through the rounds. We can see their values in the simulator, but this information is not normally visible.

When bids are tied, the simulator will pick the bidder with the alphabetically earlier name as the high bidder. This keeps the exact results predictable for our analysis, but in real auctions this is randomized.

Try swapping Nina and Omar's values. Omar now wins, but at $110 instead of $100. Since Nina is always picked as the high bidder in the first round, she and Omar alternate as high bidder each round, and Omar ends up being the one to place the final bid one increment above Nina's value.

The size of the bid increment thus determines how much error there is in the final prices. If you change the bid increment to $5, you'll see that Omar now wins the bike for $105. If you change it to $1, Omar wins for $101.

## Cooperative Auctions

In cooperative auctions, there isn't a separation between buyers (Nina, Omar) and sellers (Pat). Instead, each member of the community has an equal right to the auction proceeds, while also participating in the auction itself.

Alex and Ben are housemates, and they need to decide who is going to have the bigger room, and at what price. They both have an equal claim to it, and they use a cooperative auction to determine who gets it and what the payment is. They each bid according to what they'd pay per month to have the bigger room.

<!-- @@section:after_single_room_auction -->

The cooperative auction is the same as a regular auction, but with an extra **equalization** step. Here, the equalization step takes the auction proceeds and redistributes them to Alex and Ben equally. Knowing that they'll get half of their payment back, each bids twice what they would be willing to pay the other person to have the room.

Alex wins the space for a net payment of $25, whereas Ben is excluded from the space but gains $25. Alex obtains the space at a loss of $50 relative to if he hadn't won the space.

In cooperative auctions, your bid is the wealth gap you're willing to accept, relative to not winning. If you win, you pay the final bid price and receive your share of redistribution, and your net position relative to non-winners equals your bid.

This interpretation of bidder values holds true even as the number of bidders and the number of spaces increase. Try adding more bidders. Since these new bidders are treated as equal community members, they also obtain an equal share of the proceeds. Alex's relative loss of wealth versus the non-winning baseline remains $50 in all cases.

Each person's balance reflects their share of the proceeds minus their price. Alex's negative balance means he owes Ben and pays more rent. If their rent is $2,000, an equal split is $1,000 each. With adjustments applied, their rent payments are $1,025 and $975.

Notice how Ben is assigned no room. This is because the assignment of the smaller room is clear from the auction result. However, we can make this explicit by adding the smaller room as an option in the auction. Try adding it as an additional room. Neither person bids for it, because they have no value for it. But if you assign them a value of zero for the smaller room, you'll see that Ben bids for it only after the price for the bigger room has climbed past his value for it.

## Revaluation

People's values for spaces can change over time, so it's best to keep possession time-bounded and hold another auction at the period end. When resources have ongoing value to the user, auctions shouldn't be one-time events, but an ongoing governance mechanism. For roommates, a good cadence could be annual auctions.

For example, Ben might decide at some point that he actually values the bigger room more than he's being compensated for. Or Alex might decide that he's overpaying for the bigger room and would want to bid less for it next time.

Just because the auction is held again does not mean that Alex and Ben are going to need to switch rooms. It's likely that Alex's value for the room is still higher than Ben's, especially considering how Alex will be willing to bid more for it by the perceived cost of moving, and Ben will bid less by his perceived cost of moving. The switching costs add some stickiness to the allocation, even when the auctions are recurring.

## Unequal Bidder Wealth

You might be wondering if equal redistribution gives wealthier members an unfair advantage.

Typically, in the status quo, someone just *has* the resource, often by inertia or social dynamics, and no one else gets anything. Compared to this baseline, the compensation that the rest of the community receives from auction settlement is a net gain.

Better yet, equality of access is preserved. If people always use their compensation to bid for the resource in later auctions, then they will be able to access their equal share of the resource over time. Wealthier individuals might introduce money into the system at a higher rate, but every auction pushes balances to equalize.

The only way access becomes unequal is if someone *chooses* to spend their distributions on something else, which means they preferred that outcome. Nobody is worse off than the no-auction baseline, and most people are better off. Cooperative auctions achieve a net increase in satisfaction because they allow people to give up what is less desired for what is more desired.

## Multi-item Auctions

A year later, Alex and Ben take on a third housemate, Cam, and move into a larger place with three bedrooms that differ in size. One is large, one is medium-sized, and one is small. They hold an auction and each person expresses how much they'd pay for each room.

<!-- @@section:after_three_room_sequence_auction -->

Now that there's more than one desirable space, the method of choosing what to bid on starts to matter. In the simulator, bidders are modeled as wanting to get the best deal, defined as the largest difference between the bidders' value for a space and its current price. This difference is called the bidder's **surplus** for a space at a given price. Bidders do not always behave this way, and real ascending auctions have time during each round so bidders can change their values in response to new information or manually place their bids.

This flexibility to shift bidding between rooms during the auction is why all the rooms are auctioned together. Separate auctions for each room wouldn't allow the bidders to properly respond to competition.

Watch how bidders shift between rooms. In round 4, Cam abandons the large room: a bid there would cost $40 for $20 of surplus, while the medium room is still free and worth $30 to him. Ben does the same in round 5, switching to the medium room where $10 buys him $70 of surplus, better than the $60 surplus from continuing with the large room. When surpluses tie, the tiebreaker favors the higher-valued room, which is why Cam returns to the large room in round 6.

The rest of the auction plays out the same way: bidders pursue whichever room currently offers the most surplus. Ben eventually settles on the medium room, and Cam on the small room once nothing more valuable is cheap enough. The auction ends in round 11 when no one places any new bids.

The final allocation is Alex taking the large room for $60, Ben taking the medium room for $30, and Cam taking the small room for $0. The proceeds are $90, and divided equally each person receives a $30 credit.

Notice how Ben wins the medium room for $30, which is Cam's value for the room. But when Alex wins the large room, it's only for a value of $60, well below Ben's value of $110. This is because Ben was able to win the medium room for a reasonable price from his perspective, and he dropped out of bidding for the large room before it got as high as his value for it. If you change Cam's value for the medium room to be higher, like $40, you'll see that he bids for the medium room longer, preventing Ben from winning the medium room at the previous price, and leading to an additional round of bidding on the large room, which pushes its price from $60 to $80.

As for the final payments, we can see the interpretation of the prices as the relative difference in wealth between winners and non-winners. With payments of $60 and $30, the proceeds are $90 and the equal share is $30. Cam, paying nothing in the auction, gains this payment. Alex pays $60 and gets back $30, for a net loss of $30 and a relative loss of $60 compared to Cam. Ben's $30 payment is exactly offset by the $30 distribution, and his net payment of $0 leaves him with exactly $30 less than Cam.

This interpretation holds true regardless of the overall payment level. Try setting Cam's value for the medium room to $0: Ben wins it for $0, total proceeds fall to $60, and the equal share drops to $20. But Alex still ends up exactly $60 behind Ben and Cam. Competition changes the payment level, but it doesn't change the wealth gap the prices encode.

Everything gets more interesting at scale. Here's a larger auction. Try stepping through it and watching how bidders reposition as prices climb.

<!-- @@section:after_large_auction -->

## Competition and Prices

In the Alex, Ben, and Cam example, the rooms followed a consistent sequence of desirability. The large room is worth the most, followed by the medium room, and then by the small room. But rooms can have differences other than size, and these differences can affect which room each person values most highly. To the extent that bidders don't compete for the same things, prices do not rise.

For example, suppose that Cam actually wants the small room because it's the quietest. Now the only bidding activity is on the large room, and after Ben switches to the medium room, bidding stops.

<!-- @@section:after_three_room_less_competition_auction -->

The price on the medium room did not need to exceed zero since there wasn't competition for it. And since it was cheaper, there was less competition for the large room as well. Alex pays $26.67 on net whereas Ben and Cam receive $13.33 on net.

If Alex and Ben were the only ones interested in the large room, then why did Cam get paid? One answer is that Cam still has an equal claim to the large room. Another answer is that if we determine distributions based on each bidder's demand, then that creates a strong incentive to falsely over-report valuations. Cam doesn't need to pretend that he's interested in the large room because he knows he'll get a share of its value anyways.

## Equalization with Equal Allowances

An alternate option to equal redistribution is to equalize up front by giving each member an equal allowance of internal points. The points are issued out of the community treasury on a consistent schedule and winning bids return these points to the treasury.

This method is useful when the community doesn't want to use real money. However, points only retain value when auctions are recurring. Equal redistribution works for one-off auctions since settlement in real money directly resolves the unequal access. But without future auctions to spend points in, the value of points can collapse.

For example, a workplace can use points to allocate desks, offices, and conference rooms, with auctions occurring every quarter or year for desks, and every day or week for conference room times. People can save up points to access a high value space when they need it, or spend more consistently on less contested spaces.

Here's an example of a desk auction where the community uses "credits" that are issued on a recurring basis.

<!-- @@section:after_desk_auction -->

## The Exposure Problem

Ascending auctions are optimal only if bidders don't have preferences that are interdependent between items.

As an example of this interdependence, suppose that two coworkers want to sit next to each other, and so bid for adjacent desks. They might initially become high bidders for a pair of desks before facing competition. When outbid, they can switch to bidding for different desks, but this may only happen for one desk at a time. Thus they might only be able to release one of the desks, leaving them with a desk they don't want. In ascending auctions this phenomenon is called the **exposure problem**.

One solution in ascending auctions is to combine some items together in bundles that are bid on as a unit. Then the people that want a pair of desks are able to bid for those bundles without the risk of the exposure problem.

However, this system isn't perfect. Bidders may want different desks than the ones that are bundled. Or individual bidders may want only one of the desks in the bundle. The community has to decide what bundles best reflect the bidders' preferences.

## Decision Auctions

Another application of cooperative auctions is delegating decisions. In decision auctions, the thing available for auction is the right to make a decision on behalf of the group. This decision-making right is like a scarce resource which the community collectively owns.

For example, what movie to watch, or what restaurant to eat at, are decisions that can be resolved with cooperative auctions. Those who feel strongest about the outcome bid more for the decision right, and compensate everyone else for denying them the opportunity to have their choice.

Compared to voting, decision auctions are fairer in that non-winners of the decision receive compensation for being denied their choice. On the flip side, voting always favors the majority's preferred outcome and leaves the minority without recourse.

## Nonzero Starting Prices

In chore auctions, people compete to bid the lowest price to do a task on behalf of the community. Chore auctions set starting prices for items negative, so that winning the auction for the chore means receiving payment from the community to do it. As the auction progresses, payment climbs towards zero, representing a smaller and smaller reward, until there is only one bidder remaining.

Setting starting prices to a positive value is also useful if a space has value when unallocated to a particular person. For example, a desk might be day-use by default, unless someone is willing to pay at least some minimum price to reserve it for themselves. Or, a common area can be reserved for events, but only if a minimum payment is reached. The community can tune this minimum to determine how often the resource gets reserved instead of remaining common.

## Do it Yourself

TinyLVT exists to make cooperative auctions easier to use. To run a cooperative auction with your group, see the [setup documentation](/docs). All usage below 50 MB of storage is free. Support for nonzero starting prices is coming soon.

## Terminology and Further Reading

An ascending auction for a single item is called an **English auction**. An ascending auction for multiple items is called a **Simultaneous Ascending Auction (SAA)**.

When a bidder values a set of items more than the sum of the values of the individual items, their valuation is called **superadditive** (or the items are **complements**). Ascending auctions do not handle superadditive valuations well. The auctions that do are combinatorial auctions, but since the space of solutions grows rapidly with the auction size, they quickly become computationally intractable.

The combinatorial auction that preserves truthful bidding as a dominant strategy even when items are complements is the **Vickrey-Clarke-Groves (VCG) auction**. When bidder demand is not superadditive, SAAs approximate VCG auctions.

Decision auctions are related to **quadratic voting**, which is a voting system where people can choose where to allocate their voting budget across the issues they care about. The cost of each additional unit of preference expression on an issue scales quadratically, which makes the marginal cost of expressing a strong preference higher than expressing several mild ones. This creates incentives for coalitions to trade votes across issues. Decision auctions instead have a linear cost, so participants do best by bidding on the issues they care about directly.

Auction revenue **redistribution mechanisms** are studied for their incentive properties (Cavallo, Guo-Conitzer). The standard construction redistributes counterfactual revenue, which is strategyproof but redistributes zero when the number of bidders equals the number of items plus one. TinyLVT uses simple equal-split instead, which achieves full budget balance at all group sizes at the cost of a minor incentive to over-report: when there's a gap between the highest bidder's value and the second-highest bidder's value, a losing bidder can grow their rebate by bidding in this range, above their true value, as long as they stay below the winner's. The tradeoff favors full redistribution in small communities where budget balance matters more than perfect incentive compatibility.
