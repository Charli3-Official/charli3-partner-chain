# Oracle node onboarding

After finishing the setup as either a block producer or permissioned candidate, each Partner Chain node needs to set up a key for the Oracle.
This key will identify each node's Oracle feed, and will be used to associate the node to a Cardano address, that will be the one receiving rewards if the node is elegible for it.

1. Enter the [Polkadot/Substrate portal](https://polkadot.js.org/apps/#/explorer) and connect to the RPC endpoint of your node
2. Go to Developer -> RPC
3. Select the `Author` endpoint and then the `ìnsertKey` method
4. There are three fields to complete:
    - **keyType**: `orac`, which corresponds to the oracle pallet's crypto key type
    - **suri**: this is the public keys seed phrase which you can find in the `partner-chains-phrases-backup.json` file, here make sure to use the sr25519/AURA key, which should be the same as the one provided to the Chain builder at the start of the process
    An example suri phrase is: "bottom drive obey lake curtain smoke basket hold race lonely fit walk"
    - **publicKey**: this is the public key that was provided to the chain builder. An example sr25519 public key is: "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"

After completing hit submit, and shortly the node will start providing an Oracle feed.