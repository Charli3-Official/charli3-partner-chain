use crate::chain_spec::*;
use charli3_oracle_core::types::config::node::{ChannelId, MessagesConfiguration, TradePair};
use sc_service::ChainType;
use sidechain_runtime::{
	AccountId, AuraConfig, BalancesConfig, GrandpaConfig, NativeTokenManagementConfig,
	OracleConfig, RuntimeGenesisConfig, SessionCommitteeManagementConfig, SessionConfig,
	SidechainConfig, SudoConfig, SystemConfig,
};

use sp_core::ConstU32;
use sp_runtime::BoundedVec;
use std::str::FromStr;

/// Produces template chain spec for Partner Chains.
/// This code should be run by `partner-chains-cli chain-spec`, to produce JSON chain spec file.
/// `initial_validators` fields should be updated by the `partner-chains-cli chain-spec`.
/// Add and modify other fields of `ChainSpec` accordingly to the needs of your chain.
pub fn chain_spec() -> Result<ChainSpec, envy::Error> {
	// complete here with the corresponding keys
	let endowed_accounts: Vec<AccountId> = [
		AccountId::from_str("0x3ef249689a1f0479ebf5ff2ee2048028fb9af078e6066d9baa106133dc0f5421")
			.unwrap(),
		AccountId::from_str("0xe1480dc86820ab64a4ee2ab3eb212fd30f40afe05e4ef52fdfffc4d9d557f06f")
			.unwrap(),
		AccountId::from_str("0xafd864fc5bd15c7acc6ff76836de5232ca7e6b1e5a772a50d0aedb8cff9298b2")
			.unwrap(),
		AccountId::from_str("0x6c0f4f7880ef5be388f45f9caad909f0fb1cfd3562c7c4ca1461d25e012d4878")
			.unwrap(),
	]
	.to_vec();

	let oracle_authorized_nodes = vec![
		AccountId::from_str("0x6c0f4f7880ef5be388f45f9caad909f0fb1cfd3562c7c4ca1461d25e012d4878")
			.unwrap(),
		AccountId::from_str("0xe1480dc86820ab64a4ee2ab3eb212fd30f40afe05e4ef52fdfffc4d9d557f06f")
			.unwrap(),
	]
	.try_into()
	.expect("Authorized node exceed limit");

	let oracle_trade_pairs = BoundedVec::try_from(vec![
		TradePair::from_ticker("ADA-USD"),
		TradePair::from_ticker("STUFF-USD"),
		TradePair::from_ticker("WMT-USD"),
		TradePair::from_ticker("USDM-ADA"),
		TradePair::from_ticker("STRIKE-ADA"),
	])
	.expect("Oracle trade pairs within limit");
	let channel_id = |hex_str: &str| -> ChannelId {
		let bytes = hex::decode(hex_str).expect("Invalid hex string");
		ChannelId::try_from(bytes).expect("Channel id within limit")
	};
	let oracle_channel_mappings: MessagesConfiguration = MessagesConfiguration::try_from(vec![
		(
			channel_id("d83063f2c65eed65f307d7cd39798334633ceb5bbd29ef9b84f946e9"),
			BoundedVec::<u16, ConstU32<64>>::try_from(vec![0u16, 1u16, 2u16, 3u16, 4u16])
				.expect("Trade pair indexes within limit"),
		),
		(
			channel_id("56bd86ffdff6793f876cde8239dd3e7f3aeae333ee454d0a79ace928"),
			BoundedVec::<u16, ConstU32<64>>::try_from(vec![0u16])
				.expect("Trade pair indexes within limit"),
		),
	])
	.expect("Channel mappings within limit");

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
					"0xead402cbe090c77646376fcefe0bdb56087a5860bbc430aa1b4079fed50d199d",
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
			authorized_nodes: oracle_authorized_nodes,
			feed_age: 15,
			outliers_range: 2,
			divergency: 15,
			trade_pairs: oracle_trade_pairs,
			channels_to_trade_pairs: oracle_channel_mappings,
			reward_policy_id: Some(channel_id(
				"d83063f2c65eed65f307d7cd39798334633ceb5bbd29ef9b84f946e9",
			)),
			reward_asset_name: Some(
				BoundedVec::try_from(b"Charli3".to_vec()).expect("Asset name within limit"),
			),
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
