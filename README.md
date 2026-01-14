# Charli3 Substrate Partner Chains

This repository is Charli3's extension of [IOG's Base Partnerchain](https://github.com/input-output-hk/partner-chains) with oracle-specific customized implementation. It utilizes the charli3-oracle-core pallet to add price feed aggregation and related functionalities.

## Development environment

### Setup services for the Substrate node

We use the [docker compose](dev/docker-compose.yml) file for running the services that are needed for the Substrate node.

```
docker compose up
```
For turning them off while reseting the created blockchain, use `docker compose down --volumes`.

### Setup the Substrate node

Build the Partner Chains node from source with:
```
cargo build
```
And then you can follow the instructions on [docs/user-guides](docs/user-guides) for setting up the Substrate nodes for the different roles you want to.

1. [Chain Builder](./docs/user-guides/chain-builder.md)
2. [Permissioned Validator](./docs/user-guides/permissioned.md)
3. [Registered Validator](./docs/user-guides/registered.md)

## Configuration

### Local Testnet

As in this context we use a local testnet as main blockchain we can configure the chain parameters. The configuration for all the services can be found in the [`configurations`](./dev/configurations/) directory. To configure the testnet, we'll focus on the `cardano` and `genesis` folders.

Within `genesis`, for example, one could want to modify the epoch length, or the slot length of the main chain. In this case, we can go to the [`genesis.json`](./dev/configurations/genesis/shelley/genesis.json) within the `shelley` folder in `genesis`.
At the beginning of the file, we have the `epoch length` field, which originally is set to `120`.

https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/55e28c318056fdf9029e9e1722196e162454a8c2/dev/configurations/genesis/shelley/genesis.json#L1-L4

By replacing that value, we can modify the length of the epochs in the local testnet. Note that here the `epoch` length is measured in amount of `slots`.

Further in this same file, around line 53, we can find the `slot length` field, that is set at `1` initially.

https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/55e28c318056fdf9029e9e1722196e162454a8c2/dev/configurations/genesis/shelley/genesis.json#L52-L55

Changing this value will result in a modification of the slot length in the local testnet. Note that the slot length is measured in `seconds`.

Another detail we might want to change is the initial funds distribution. In the `entrypoint.sh` file within the `configurations/cardano` directory, we can find the inital set up of the node. One of the inital configurations is a transaction that creates utxos for some addresses.

https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/55e28c318056fdf9029e9e1722196e162454a8c2/dev/configurations/cardano/entrypoint.sh#L112-L124

This command can be modified to pay out to other addresses by adding a line like this:
```bash
--tx-out "$my_address+$my_amount" \
```
where `my_address` is the desired recipient in Bech32 format, and `my_amount` is the desired amount of lovelace.
These two variables need to be defined previously, a good place to declare them is below the definition of the output amounts:

https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/55e28c318056fdf9029e9e1722196e162454a8c2/dev/configurations/cardano/entrypoint.sh#L96-L106

And the `my_amount` also needs to be added to the following line, where the final output amount is calculated so the transaction is balanced:

https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/55e28c318056fdf9029e9e1722196e162454a8c2/dev/configurations/cardano/entrypoint.sh#L109

If it's not added, the transaction won't be submitted correctly.

## Features

### Block Production Rewards for Validators

The Partner Chains node provides a simple mechanism for exposing the mapping of block beneficiaries with produced blocks. 
The chain builder is responsible for using this input to accurately calculate and distribute block rewards on their partner chain, following the tokenomics they will design and implement. 
For managing payouts, they will leverage the provided artifacts to distribute payments to validators on the partner chain ledger, applying their specific business logic.

| Component | Description |
|-----------|-------------|
| Registration | Validators register their Cardano cold secret key with their Partner Chain keys by producing two signatures and providing the public keys of each (Cardano and Partner Chains) so that others can verify that the signature matches the public key |
| Block logging | Partner Chain nodes log block production data, including beneficiaries |
| Reward calculation | Determined by the Partner Chain instance (for example, N tokens per block) |
| Smart contracts | Track permissioned candidates, and registered candidates |

#### For Partner Chains Node Operators

1. Implement a reward distribution system using the block production data provided by the Partner Chains node. 
2. Set up and manage the required smart contracts on Cardano.
3. Automate the reward calculation and distribution process within the consensus layer.
4. Establish a registration process for validators.

#### For Stake Pool Operators (SPOs)

- Register to become a partner chain validator.
- Monitor block production and reward distribution for your pool.

#### More Details

1. Block Production Tracking: The Partner Chain node logs block production data. Each block has a beneficiary, identified by Partner Chain receiving addresses (such as `SizedByteStrings(0x1)`, `SizedByteStrings(0x2)`, etc.). At the end of each Partner Chain epoch, the node summarizes who produced how many blocks.
2. Smart Contract Implementation: Two main smart contracts are involved: 

    a. Permissioned candidates contract: Lists approved validators with their public keys.

    b. Registered candidates contract: Lists registered SPOs who can act as validators.

3. Reward Distribution Mechanism: The Partner Chain instance uses the block production data from the node logs and the information from the smart contracts to determine the reward distribution. It then implements the transactions to send the appropriate amount of rewards to each validator.
4. Automation and Verification: The entire process needs to be automated within the consensus layer of the Partner Chain. Automated tests will be implemented to verify that the reward distribution is functioning correctly over time.

### Native Token Management

When establishing a partner chain managing the token treasury used for block production rewards and other applications is a central part of leveraging the Cardano network as the root of trust, as one of the primary objectives of a possible adversarial group attacking a partner chain might be to get access to the token treasury.

As such partner chain developers may want to use a model where their token exists “canonically” on multiple chains. In this case it is paramount that the circulating and total supply of the token is maintained and acting predicatably, even with the release of tokens into circulation on the partner chain as a part of block production rewards, or services rendered.

The mechanism for this includes a partner chain builder initializing a token reserve on Cardano, along with a circulating supply on both chains. The circulating tokens exist in a locked, or unlocked state, and the relation between the locked state on Cardano and unlocked on the partner chain should be 1:1 and vice versa.

This means that tokens are moved from the reserve into a locked circulating supply on Cardano, at which point they become available in circulation on the partner chain either through a time direct observability.

For more details on how to implement Native Token Management in a partner chain, refer to the [Native Token Migration Guide](docs/developer-guides/native-token-migration-guide.md)

## License

This repository is part of the Charli3 Oracle Partnerchain Core System and is
licensed under the Business Source License (BSL) 1.1.

What this allows:

- ✅ Full source visibility and auditability
- ✅ Internal and production use with the Charli3 hosted partner-chain
- ✅ Development of oracle templates, bridges, and data adapters
- ✅ Commercial sale of templates and adapters on the Charli3 marketplace
- ✅ Limited modification of the core to support extensions

What this restricts:

- ❌ Self-hosting or operating a competing partner-chain or oracle network
- ❌ Offering oracle-network-as-a-service using this software without a
  commercial license

Each release automatically becomes MIT licensed after 18 months.

For commercial licensing inquiries:
📧 sales@charli3.io
