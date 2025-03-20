# Reward distribution

A decentralized oracle network utilizes rewards to encourage good behavior from the Oracle nodes. Good behavior in this context implies supplying accurate information in a timely manner.

In the Charli3 Oracle Partner Chain the reward candidates are obtained through the aggregation process. In this process, as described in the design document, a list of non outlier prices, according to the median, is curated from all the valid prices for the given block.  

## Desirable behavior

### Regular updates

The nodes are expected to provide a new price for each block, to ensure that the aggregation is always calculated with the newest information available. The chain builder can configure for how long a feed can go without being updated until it is not considered *new* anymore. Since the blocks of the partner chain are produced very quickly, one could allow feeds to be a few blocks old without losing recency. The feed age parameter is used to determine the recency.
A node that does not update a feed for a predefined amount of blocks will not participate in the aggregation process and thus it will not be eligible to receive a reward for that block.

### Proximity to the median

The next important factor is the feed in itself that the node provides. This feed needs to be within a range of divergence from the median to be considered for rewards. This helps filter out a malicious node that provides an extremely different value from the rest. The outliers range and divergence percentage are preconfigured parameters that are used to determine this. The outliers range is used to identify outliers, limited to a percentage of variation from the predefined divergence percentage.

## Implementation

### Outlier filtering algorithm

The reward system in this case relies on the implementation of a simple filtering algorithm that incorporates the pre configured parameters mentioned above and the median to calculate outliers. These parameters are stored on the partner chain’s global state at the time of the genesis, and they are configurable because these values can be very different for different feeds.

The algorithm in itself is based on a common statistical technique to identify outliers using the interquartile range (IQR) and the median. It follows the next steps:
Calculate the first and third quartiles, and use them to obtain the interquartile range
Use the IQR and the predefined outliers range to calculate the outlier bounds
Filter outliers according to:
The outlier bounds
The divergence percentage, to ensure that a non-outlier value is relatively close to the median

 ### Partner chain accounts to Cardano address

Another implementation aspect, although not intrinsic to the reward algorithm, is a storage that maps accounts in the partner chain to Cardano addresses. There is one partner chain account for each node. So this maps nodes to addresses as well. This allows the partner chain to identify which Cardano addresses could be awarded rewards, and communicate this in a manner that is straightforward for the users.

## Reward candidates

The chain builder might decide that the rewards won’t be paid for every block on the partner chain, as this would mean a lot of transactions happening quickly on the Cardano side, but rather they can be awarded periodically based on the amount of times each node has appeared on the reward_elegible_node list. Because this is a business logic decision that may vary from feed to feed, the algorithm implemented informs a list of reward eligible nodes or reward candidates, this way the chain builder is capable of defining when and how the rewards are paid out.
