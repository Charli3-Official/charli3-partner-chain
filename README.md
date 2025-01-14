# Charli3 Substrate Partner Chains

Our own Partner Chains repository version for Charli3.

## Development environment

### Setup services for the Substrate node

We use the [dev/local-environment setup](dev/local-environment/setup.sh) for creating the Docker compose file for running the services that are needed for the Substrate node.

```
cd dev/local-environment
./setup.sh --deployment-option 3 --postgres-password pass
```
The `--deployment-option 3` is the number of the deployment option we need: Cardano Node on a local configured testnet, Cardano DB Sync, Postgres, Kupo and Ogmios.

The `--postgres-password pass` is the password for the Postgres database. Could be any password.

Then, with `docker compose up`, we can start the services. For turning them off, use `docker compose down --volumes`.

### Setup the Substrate node

Build the Partner Chains node from source with the following command:
```
cargo build --profile=production
```
And then you can follow the instructions on [docs/user-guides](docs/user-guides) for setting up the Substrate nodes for the different roles you want to.

1. [Chain Builder](./docs/user-guides/chain-builder.md)
2. [Permissioned Validator](./docs/user-guides/permissioned.md)
3. [Registered Validator](./docs/user-guides/registered.md)


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
