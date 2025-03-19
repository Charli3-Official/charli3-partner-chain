# Block-producing committee

The block–producing committee is created by the _chain builder_ as part of the Substrate blockchain setup and configuration. The committee is comprised of a set of _permissioned candidates_ and _registered candidates_.

When creating the Substrate chain, the chain builder is in charge of defining two important things: the quantity of permissioned and registered candidates, both expressed in the D-Parameter, and which are the permissioned candidates, identified by their Substrate public keys.

The permissioned candidates are Substrate node validators who have to be white-listed by the chain builder to become block producers on the partner chain.

Registered candidates, on the other hand, are SPOs that by registering to the Partner chain can process and validate transaction blocks in the Substrate chain with their Substrate node.

## Registration of candidates
As mentioned above, SPOs use the `partner-chains-cli` to register for helping secure the Partner Chain network by processing and validating blocks.

The registration is a three-step process using the Partner Chain CLI
- `registrer1`: builds the transaction leaving blank space for the SPO cold key signature.
- `register2`: should be made in the offline machine where the cold key is placed. Here the transaction is signed and returned back to an online machine for completing the registration.
- `register3`: submits the registration transaction

The result of the registration is a Registration UTxO that contains in its datum public keys for the SPO and the related Substrate node.

## Authority Selection

On a given Substrate blockchain epoch, a subset of the candidates are pseudo-randomly selected for producing and validating blocks. This selection occurs at the end of each epoch, where the committee rotation takes place. In the case of registered candidates, the more stake their pools have delegated, the more chances of being selected they have.
