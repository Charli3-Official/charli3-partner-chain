use crate::chain_spec::*;
use sc_service::ChainType;
use sidechain_runtime::{
	AccountId, AuraConfig, BalancesConfig, GrandpaConfig, NativeTokenManagementConfig,
	OracleConfig, RuntimeGenesisConfig, SessionCommitteeManagementConfig, SessionConfig,
	SidechainConfig, SudoConfig, SystemConfig,
};
use sidechain_domain::{MainchainAddress};
use std::str::FromStr;

/// Produces template chain spec for Partner Chains.
/// This code should be run by `partner-chains-cli chain-spec`, to produce JSON chain spec file.
/// `initial_validators` fields should be updated by the `partner-chains-cli chain-spec`.
/// Add and modify other fields of `ChainSpec` accordingly to the needs of your chain.
pub fn chain_spec() -> Result<ChainSpec, envy::Error> {
	// complete here with the corresponding keys and addresses
	let sr25519_key_and_mainchain_addresses: Vec<(&str,  &str)> = [
		("0x...", "stake_test...")
	].to_vec();
	let accounts_and_mainchain_addresses: Vec<(AccountId, MainchainAddress)> = sr25519_key_and_mainchain_addresses.iter().cloned().map(
		|(sr25519_key, mc_address)| (AccountId::from_str(sr25519_key).unwrap(), MainchainAddress::from_str(mc_address).unwrap())
	).collect();
	let runtime_genesis_config = RuntimeGenesisConfig {
		system: SystemConfig { ..Default::default() },
		balances: BalancesConfig {
			// Update if any endowed accounts are required.
			balances: accounts_and_mainchain_addresses.iter().cloned().map(|(k, _)| (k, 0)).collect(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![], ..Default::default() },
		sudo: SudoConfig {
			// No sudo account by default, please update with your preferences.
			key: None,
		},
		transaction_payment: Default::default(),
		session: SessionConfig {
			// Initial validators are meant to be updated in the chain spec file, so it is empty here.
			initial_validators: vec![],
		},
		sidechain: SidechainConfig {
			genesis_utxo: sp_sidechain::read_genesis_utxo_from_env_with_defaults()?,
			..Default::default()
		},
		pallet_session: Default::default(),
		session_committee_management: SessionCommitteeManagementConfig {
			// Same as SessionConfig
			initial_authorities: vec![],
			main_chain_scripts: sp_session_validator_management::MainChainScripts::read_from_env()?,
		},
		native_token_management: NativeTokenManagementConfig {
			main_chain_scripts: sp_native_token_management::MainChainScripts::read_from_env()?,
			..Default::default()
		},
		oracle: OracleConfig {
			min_nodes_for_trusted_aggregation: 1,
			feed_age: 15,
			outliers_range: 2,
			divergence_percentage: 15,
			orac_key_to_mainchain_address: accounts_and_mainchain_addresses,
			..Default::default()
		},
	};
	let genesis_json = serde_json::to_value(runtime_genesis_config)
		.expect("Genesis config must be serialized correctly");
	Ok(ChainSpec::builder(runtime_wasm(), None)
		.with_name("Partner Chains Template")
		.with_id("partner_chains_template")
		.with_chain_type(ChainType::Live)
		.with_genesis_config(genesis_json)
		.build())
}
