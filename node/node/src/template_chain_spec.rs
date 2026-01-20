use crate::chain_spec::*;
use sc_service::ChainType;
use sidechain_runtime::{
	AccountId, AuraConfig, BalancesConfig, GrandpaConfig, NativeTokenManagementConfig,
	OracleConfig, RuntimeGenesisConfig, SessionCommitteeManagementConfig, SessionConfig,
	SidechainConfig, SudoConfig, SystemConfig,
};
use std::str::FromStr;
use sp_consensus_aura::ed25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
}

/// Helper to create a multisig account from signers + threshold
pub fn get_multisig_account(signers: Vec<AccountId>, threshold: u16) -> AccountId {
	// Must be sorted!
	let mut sorted = signers;
	sorted.sort();
	pallet_multisig::Pallet::<sidechain_runtime::Runtime>::multi_account_id(&sorted, threshold)
}

/// Produces template chain spec for Partner Chains.
/// This code should be run by `partner-chains-cli chain-spec`, to produce JSON chain spec file.
/// `initial_validators` fields should be updated by the `partner-chains-cli chain-spec`.
/// Add and modify other fields of `ChainSpec` accordingly to the needs of your chain.
pub fn chain_spec() -> Result<ChainSpec, envy::Error> {
	// complete here with the corresponding keys
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
