use crate::chain_spec::*;
use partner_chains_runtime::{
	AccountId, AuraConfig, BalancesConfig, BridgeConfig, GovernedMapConfig, GrandpaConfig,
	OracleConfig, RuntimeGenesisConfig, SessionCommitteeManagementConfig, SessionConfig,
	SidechainConfig, SudoConfig, SystemConfig, TestHelperPalletConfig,
};
use sc_service::ChainType;
use std::str::FromStr;

/// Produces template chain spec for Partner Chains.
/// This code should be run by `partner-chains-node wizards chain-spec`, to produce JSON chain spec file.
/// `initial_validators` fields should be updated by the `partner-chains-node wizards chain-spec`.
/// Add and modify other fields of `ChainSpec` accordingly to the needs of your chain.
pub fn chain_spec() -> Result<ChainSpec, envy::Error> {
	let genesis_utxo = sp_sidechain::read_genesis_utxo_from_env_with_defaults()?;
	let endowed_accounts: Vec<AccountId> = [
		AccountId::from_str("0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")
			.unwrap(),
		AccountId::from_str("0xac1d6fb8dd38138a2167493fecb2b5d49bb8a087a9956a53e0357eedb857f658")
			.unwrap(),
	]
	.to_vec();
	let runtime_genesis_config = RuntimeGenesisConfig {
		system: SystemConfig { ..Default::default() },
		balances: BalancesConfig {
			// Update if any endowed accounts are required.
			balances: endowed_accounts.iter().cloned().map(|k| (k, 0)).collect(),
			dev_accounts: None,
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![], ..Default::default() },
		sudo: SudoConfig {
			// No sudo account by default, please update with your preferences.
			key: Some(
				AccountId::from_str(
					"0xb13b1465adee39623aa3f493f9d2c0c6c9a01f7723cf081488e01ff8da617318",
				)
				.unwrap(),
			),
		},
		transaction_payment: Default::default(),
		session: SessionConfig {
			// Initial validators are meant to be updated in the chain spec file, so it is empty here.
			initial_validators: vec![],
		},
		sidechain: SidechainConfig { genesis_utxo, ..Default::default() },
		pallet_session: Default::default(),
		session_committee_management: SessionCommitteeManagementConfig {
			// Same as SessionConfig
			initial_authorities: vec![],
			main_chain_scripts: sp_session_validator_management::MainChainScripts::read_from_env()?,
		},
		governed_map: GovernedMapConfig {
			main_chain_scripts: Some(sp_governed_map::MainChainScriptsV1::read_from_env()?),
			..Default::default()
		},
		test_helper_pallet: TestHelperPalletConfig {
			participation_data_release_period: 30,
			..Default::default()
		},
		bridge: BridgeConfig {
			main_chain_scripts: Some(sp_partner_chains_bridge::MainChainScripts::read_from_env()?),
			initial_checkpoint: Some(genesis_utxo),
			..Default::default()
		},
		oracle: OracleConfig {
			min_nodes_for_trusted_aggregation: 1,
			feed_age: 15,
			outliers_range: 2,
			divergency: 15,
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
