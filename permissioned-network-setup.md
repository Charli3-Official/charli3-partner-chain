# Permissioned network setup

This guide is to setup initial deployment with permissioned oracle nodes set.

## Prerequisites

### Build dependencies

You have three options for Cardano deps preparation:

1. Using [docker](https://github.com/input-output-hk/partner-chains/blob/master/docs/developer-guides/dependencies.md#using-docker) containers (the easiest one)
2. Using [nix](https://github.com/input-output-hk/partner-chains/blob/master/docs/developer-guides/dependencies.md#using-the-containerless-nix-stack) with process compose
3. Installing all deps manually - see corresponding guides in the deploy flow section

### Environment preparation

You should deploy all Cardano dependencies based on the network of your choice (preview, preprod, mainnet), for local testnet you can use this docker compose [file](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/dev/docker-compose.yml).
Cardano dependencies are:

1. cardano-node
2. ogmios
3. kupo
4. postgres
5. db-sync

See also this [guide](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/developer-guides/dependencies.md), and the original [repo](https://github.com/input-output-hk/partner-chains/blob/master/dev/local-environment/README.md).

After that is deployed wait for services startup and chain indexing, all services should be fully synchronized before you can continue with the deploy flow.

## Setup

### Build tools

You have two options for tools installation:

1. Using [rustup](https://github.com/txpipe-shop/charli3-substrate/blob/main/docs/rust-setup.md#rust-developer-environment), it uses `rust-toolchain.toml` file to auto-install all dependencies when you use rust/cargo command, use `rustup show` to explicitly trigger the installation (the easiest one)
2. Using [nix](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/developer-guides/getting-started.md#nix) with direnv, it uses `flake.nix` and `.envrc` files to auto-install all dependencies, you should use `direnv allow` to update env variables

After you done installing the tools run `cargo build --release` to build all executables. You will need to copy paths to two of them (you can build only these two if you want) `target/release/partner-chains-node` and `target/release/partner-chains-cli`.

### Deploy flow

There are two types of nodes, first is [chain-builder](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/user-guides/chain-builder.md), second is [permissioned](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/user-guides/permissioned.md) node. For every node set up you can go (cd) to the corresponding folder at `dev/configurations/partner-chains-nodes` for convenience, there are some keys pre-generated, but we don't need to use them all.

1. generate-keys for every permissioned node including chain-builder, this is "2. Run the generate-keys wizard" step from the two guides above.
You should copy the contents `partner-chains-public-keys.json` files for each node, to then register them as `initial_permissioned_candidates` inside `partner-chains-cli-chain-config.json` file.

2. setup chain builder - steps 3-6 from the [guide](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/user-guides/chain-builder.md#3-rebuild-with-the-list-of-sr25519-keys-and-cardano-addresses).
You should use `partner-chains-public-keys.json` and `partner-chains-phrases-backup.json` files generated at previous step for substrate-side keys entry, while for payment keys you can use `payment.short.addr`, `payment.skey`, `payment.vkey` files inside `dev/configurations/partner-chains-nodes/partner-chains-node-ix/keys/` folder.

3. distribute configurations - [step 5](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/user-guides/permissioned.md#5-obtain-the-chain-configuration-and-specification-files).

4. start the nodes - last steps from the two guides above.

5. [register](https://github.com/txpipe-shop/charli3-substrate-partner-chains/blob/main/docs/user-guides/oracle-usage.md) all nodes as oracles.
You should connect to corresponding rpc port, which is logged when node is first booted (e.g. `Running JSON-RPC server: addr=127.0.0.1:9944,[::1]:9944`).
After that corresponding node logs should periodically produce "submit transaction success" with their own aura sr25519 pub key.
