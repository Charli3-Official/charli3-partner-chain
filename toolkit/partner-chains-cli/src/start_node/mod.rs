use crate::config::config_fields::{
	NODE_P2P_PORT, POSTGRES_CONNECTION_STRING, SIDECHAIN_BLOCK_BENEFICIARY,
};
use crate::config::config_values::*;
use crate::config::{CardanoParameters, CHAIN_CONFIG_FILE_PATH, CHAIN_SPEC_PATH};
use crate::io::IOContext;
use crate::keystore::*;
use crate::{config::config_fields, *};
use anyhow::anyhow;
use secp256k1::PublicKey;
use serde::Deserialize;
use sp_core::crypto::AccountId32;
use sp_runtime::app_crypto::ecdsa;
use sp_runtime::traits::IdentifyAccount;
use sp_runtime::MultiSigner;
use std::{path::Path, str::FromStr};

#[cfg(test)]
mod tests;

const DEFAULT_RPC_PORT: u16 = 9944;
const ORACLE_CONFIG_RELATIVE_PATH: &str = "oracle/node-config.json";
const ORACLE_NODE_CONFIG_STORAGE_KEY: &str = "0x6e6f64655f636f6e666967";

#[derive(Debug, clap::Parser)]
pub struct StartNodeCmd {
	#[arg(long)]
	silent: bool,
}

pub struct StartNodeConfig {
	pub substrate_node_base_path: String,
	pub node_executable: String,
}

impl StartNodeConfig {
	pub fn load<C: IOContext>(context: &C) -> Self {
		Self {
			substrate_node_base_path: config_fields::SUBSTRATE_NODE_DATA_BASE_PATH
				.load_or_prompt_and_save(context),
			node_executable: config_fields::NODE_EXECUTABLE.load_or_prompt_and_save(context),
		}
	}
	pub fn keystore_path(&self) -> String {
		keystore_path(&self.substrate_node_base_path, DEFAULT_CHAIN_NAME)
	}
}

#[derive(Deserialize)]
pub struct StartNodeChainConfig {
	pub cardano: CardanoParameters,
	pub bootnodes: Vec<String>,
}

impl CmdRun for StartNodeCmd {
	fn run<C: IOContext>(&self, context: &C) -> anyhow::Result<()> {
		let config = StartNodeConfig::load(context);
		verify_config(&config, context)?;

		if !check_keystore(&config, context)? || !check_chain_spec(context) {
			return Ok(());
		}

		let db_connection_string = POSTGRES_CONNECTION_STRING.load_or_prompt_and_save(context);

		let Some(chain_config) = load_chain_config(context)? else { return Ok(()) };

		let beneficiary = SIDECHAIN_BLOCK_BENEFICIARY
			.save_if_empty(block_beneficiary_from_cross_chain_key(&config, context)?, context);
		if !self.silent
			&& !prompt_values_fine(
				&config,
				&chain_config,
				&db_connection_string,
				&beneficiary,
				context,
			) {
			context.eprint("Aborting. Edit configuration files and rerun the command.");
			return Ok(());
		}

		start_node(config, chain_config, &db_connection_string, beneficiary, context)?;

		Ok(())
	}
}

fn verify_config<C: IOContext>(config: &StartNodeConfig, context: &C) -> anyhow::Result<()> {
	if !context.file_exists(&config.node_executable) {
		return Err(anyhow!(
			"Partner Chains Node executable file ({}) is missing",
			config.node_executable
		));
	}
	Ok(())
}

fn load_chain_config<C: IOContext>(context: &C) -> anyhow::Result<Option<StartNodeChainConfig>> {
	let Some(chain_config_file) = context.read_file(CHAIN_CONFIG_FILE_PATH) else {
		context.eprint(&format!("⚠️ Chain config file {CHAIN_CONFIG_FILE_PATH} does not exists. Run prepare-configuration wizard first."));
		return Ok(None);
	};
	let chain_config = match serde_json::from_str::<StartNodeChainConfig>(&chain_config_file) {
		Ok(chain_config) => chain_config,
		Err(err) => {
			context.eprint(&format!("⚠️ Chain config file {CHAIN_CONFIG_FILE_PATH} is invalid: {err}. Run prepare-configuration wizard or fix errors manually."));
			return Ok(None);
		},
	};
	Ok(Some(chain_config))
}

#[rustfmt::skip]
fn prompt_values_fine<C: IOContext>(
	StartNodeConfig { substrate_node_base_path, node_executable }: &StartNodeConfig,
	StartNodeChainConfig {
		cardano,
		bootnodes,
	}: &StartNodeChainConfig,
	db_connection_string: &str,
	beneficiary: &str,
	context: &C,
) -> bool
{
	context.eprint("The following values will be used to run the node:");
	context.eprint(&format!("    executable = {}", node_executable));
	context.eprint(&format!("    base path  = {}", substrate_node_base_path));
	context.eprint(&format!("    chain spec = {}", CHAIN_SPEC_PATH));
	context.eprint(&format!("    bootnodes  = [{}]", bootnodes.join(", ")));
	context.eprint("    environment:");
	context.eprint(&format!("        BLOCK_STABILITY_MARGIN             = {}", 0));
	context.eprint(&format!("        CARDANO_SECURITY_PARAMETER         = {}", cardano.security_parameter));
	context.eprint(&format!("        CARDANO_ACTIVE_SLOTS_COEFF         = {}", cardano.active_slots_coeff));
	context.eprint(&format!("        FIRST_EPOCH_TIMESTAMP_MILLIS       = {}", cardano.first_epoch_timestamp_millis));
	context.eprint(&format!("        EPOCH_DURATION_MILLIS              = {}", cardano.epoch_duration_millis));
	context.eprint(&format!("        FIRST_EPOCH_NUMBER                 = {}", cardano.first_epoch_number));
	context.eprint(&format!("        FIRST_SLOT_NUMBER                  = {}", cardano.first_slot_number));
	context.eprint(&format!("        DB_SYNC_POSTGRES_CONNECTION_STRING = {}", db_connection_string));
	context.eprint(&format!("        SIDECHAIN_BLOCK_BENEFICIARY        = {}", beneficiary));
	context.prompt_yes_no("Proceed?", true)
}

fn check_chain_spec<C: IOContext>(context: &C) -> bool {
	if context.file_exists(CHAIN_SPEC_PATH) {
		true
	} else {
		context.eprint(&format!("Chain spec file {} missing.", CHAIN_SPEC_PATH));
		context.eprint("Please run the create-chain-spec wizard first or you can get it from your chain governance.");
		false
	}
}

fn check_keystore<C: IOContext>(config: &StartNodeConfig, context: &C) -> anyhow::Result<bool> {
	let existing_keys = context.list_directory(&config.keystore_path())?.unwrap_or_default();
	Ok(key_present(&AURA, &existing_keys, context)
		&& key_present(&GRANDPA, &existing_keys, context)
		&& key_present(&CROSS_CHAIN, &existing_keys, context)
		&& key_present(&ORACLE, &existing_keys, context))
}

fn key_present<C: IOContext>(key: &KeyDefinition, existing_keys: &[String], context: &C) -> bool {
	if find_existing_key(existing_keys, key).is_none() {
		context.eprint(&format!(
			"⚠️ {} key is missing from the keystore. Please run generate-keys wizard first.",
			key.name
		));
		false
	} else {
		true
	}
}

fn block_beneficiary_from_cross_chain_key(
	config: &StartNodeConfig,
	context: &impl IOContext,
) -> anyhow::Result<String> {
	let existing_keys = context.list_directory(&config.keystore_path())?.unwrap_or_default();
	let key = find_existing_key(&existing_keys, &CROSS_CHAIN).ok_or(anyhow!(
		"⚠️ {} key is missing from the keystore. Please run generate-keys wizard first.",
		CROSS_CHAIN.name
	))?;
	account_id_hex_from_ecdsa_key(&key)
}

fn account_id_hex_from_ecdsa_key(key: &str) -> anyhow::Result<String> {
	let trimmed = key.trim_start_matches("0x");
	let pk = PublicKey::from_str(trimmed)?;
	let account_id: AccountId32 = MultiSigner::from(ecdsa::Public::from(pk)).into_account();
	Ok(hex::encode(account_id))
}

fn shell_quote(input: &str) -> String {
	if input.is_empty() {
		"''".to_string()
	} else if !input.contains('\'') {
		format!("'{}'", input)
	} else {
		let escaped = input.replace('\'', "'\"'\"'");
		format!("'{}'", escaped)
	}
}

fn build_startup_script(
	env_vars: &[(&str, String)],
	node_executable: &str,
	base_path: &str,
	ws_port: u16,
	bootnodes: &[String],
	oracle_config_path: &str,
	oracle_config_hex: &str,
	rpc_port: u16,
) -> String {
	let mut script = String::new();
	script.push_str("#!/usr/bin/env sh\n");
	script.push_str("set -euo pipefail\n\n");
	for (name, value) in env_vars {
		script.push_str(&format!("export {name}={}\n", shell_quote(value)));
	}
	script.push('\n');
	script.push_str(&format!("NODE_EXECUTABLE={}\n", shell_quote(node_executable)));
	script.push_str(&format!("CHAIN_SPEC={}\n", shell_quote(CHAIN_SPEC_PATH)));
	script.push_str(&format!("BASE_PATH={}\n", shell_quote(base_path)));
	script.push_str(&format!("WS_PORT={ws_port}\n"));
	script.push_str(&format!("RPC_PORT={rpc_port}\n"));
	script.push_str(&format!("ORACLE_CONFIG_PATH={}\n", shell_quote(oracle_config_path)));
	script.push_str(&format!("ORACLE_CONFIG_HEX={oracle_config_hex}\n"));
	script.push('\n');
	script.push_str("NODE_PID=''\n");
	script.push_str("cleanup() {\n");
	script.push_str("\tif [ -n \"$NODE_PID\" ]; then\n");
	script.push_str("\t\tkill -TERM \"$NODE_PID\" 2>/dev/null || true\n");
	script.push_str("\tfi\n");
	script.push_str("}\n");
	script.push_str("trap 'cleanup; exit 0' INT TERM\n\n");
	script.push_str("set -- --validator --chain \"$CHAIN_SPEC\" --base-path \"$BASE_PATH\" --port \"$WS_PORT\"\n");
	for bootnode in bootnodes {
		script.push_str(&format!(
			"set -- \"$@\" --bootnodes {}\n",
			shell_quote(bootnode)
		));
	}
	script.push_str("\"$NODE_EXECUTABLE\" \"$@\" &\n");
	script.push_str("NODE_PID=$!\n\n");
	script.push_str("SYSTEM_HEALTH_PAYLOAD='{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"system_health\"}'\n");
	script.push_str(&format!(
		"OFFCHAIN_PAYLOAD=\"{{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"id\\\":1,\\\"method\\\":\\\"offchain_localStorageSet\\\",\\\"params\\\":[\\\"PERSISTENT\\\",\\\"{ORACLE_NODE_CONFIG_STORAGE_KEY}\\\",\\\"0x${{ORACLE_CONFIG_HEX}}\\\"]}}\"\n"
	));
	script.push('\n');
	script.push_str("printf 'Waiting for node RPC on port %s...\\n' \"$RPC_PORT\"\n");
	script.push_str(
		"until curl -s -H 'Content-Type: application/json' -d \"$SYSTEM_HEALTH_PAYLOAD\" \"http://127.0.0.1:$RPC_PORT\" >/dev/null 2>&1; do\n",
	);
	script.push_str("\tsleep 2\n");
	script.push_str("\tif ! kill -0 \"$NODE_PID\" 2>/dev/null; then\n");
	script.push_str("\t\techo 'Node process exited before RPC became available.'\n");
	script.push_str("\t\texit 1\n");
	script.push_str("\tfi\n");
	script.push_str("done\n\n");
	script.push_str("echo \"Loading oracle price configuration from $ORACLE_CONFIG_PATH.\"\n");
	script.push_str(
		"response=$(curl -s -w '\\n%{http_code}' -H 'Content-Type: application/json' -d \"$OFFCHAIN_PAYLOAD\" \"http://127.0.0.1:$RPC_PORT\")\n",
	);
	script.push_str("status=$(printf '%s' \"$response\" | tail -n1)\n");
	script.push_str("body=$(printf '%s' \"$response\" | sed '$d')\n");
	script.push_str("if [ \"$status\" = \"200\" ]; then\n");
	script.push_str("\techo 'Oracle configuration stored in offchain storage.'\n");
	script.push_str("else\n");
	script.push_str("\techo \"Failed to store oracle configuration (status $status).\"\n");
	script.push_str("\tif [ -n \"$body\" ]; then\n");
	script.push_str("\t\techo \"$body\"\n");
	script.push_str("\tfi\n");
	script.push_str("fi\n\n");
	script.push_str("wait \"$NODE_PID\"\n");
	script
}

pub fn start_node<C: IOContext>(
	StartNodeConfig { substrate_node_base_path, node_executable }: StartNodeConfig,
	StartNodeChainConfig {
		cardano:
			CardanoParameters {
				security_parameter,
				active_slots_coeff,
				first_epoch_number,
				first_slot_number,
				epoch_duration_millis,
				first_epoch_timestamp_millis,
			},
		bootnodes,
	}: StartNodeChainConfig,
	db_connection_string: &str,
	beneficiary: String,
	context: &C,
) -> anyhow::Result<()> {
	let oracle_config_path = Path::new(&substrate_node_base_path).join(ORACLE_CONFIG_RELATIVE_PATH);
	let oracle_config_path_str = oracle_config_path
		.to_str()
		.ok_or_else(|| anyhow!("Oracle price configuration path contains invalid UTF-8"))?;
	let Some(oracle_config) = context.read_file(oracle_config_path_str) else {
		context.eprint(&format!(
			"⚠️ Oracle price configuration file {oracle_config_path_str} is missing. Copy config/example-config.json into {oracle_config_path_str} and rerun."
		));
		return Err(anyhow!("oracle price configuration file missing"));
	};

	let oracle_config_hex = hex::encode(oracle_config.as_bytes());

	let ws_port = NODE_P2P_PORT.save_if_empty(
		NODE_P2P_PORT
			.default
			.expect("Default NODE_WS_PORT should always be set")
			.parse()
			.expect("Default NODE_WS_PORT should be valid u16"),
		context,
	);

	let env_vars = vec![
		("CARDANO_SECURITY_PARAMETER", security_parameter.to_string()),
		("CARDANO_ACTIVE_SLOTS_COEFF", active_slots_coeff.to_string()),
		(
			"DB_SYNC_POSTGRES_CONNECTION_STRING",
			db_connection_string.to_string(),
		),
		(
			"MC__FIRST_EPOCH_TIMESTAMP_MILLIS",
			first_epoch_timestamp_millis.to_string(),
		),
		("MC__EPOCH_DURATION_MILLIS", epoch_duration_millis.to_string()),
		("MC__FIRST_EPOCH_NUMBER", first_epoch_number.to_string()),
		("MC__FIRST_SLOT_NUMBER", first_slot_number.to_string()),
		("BLOCK_STABILITY_MARGIN", 0.to_string()),
		("SIDECHAIN_BLOCK_BENEFICIARY", beneficiary.clone()),
	];

	let start_script_content = build_startup_script(
		&env_vars,
		&node_executable,
		&substrate_node_base_path,
		ws_port,
		&bootnodes,
		oracle_config_path_str,
		&oracle_config_hex,
		DEFAULT_RPC_PORT,
	);

	let script_file = context.new_tmp_file(&start_script_content);
	let script_path_buf = script_file.to_path_buf();
	let script_path = script_path_buf
		.to_str()
		.ok_or_else(|| anyhow!("temporary script path contains invalid UTF-8"))?
		.to_string();

	context.run_command(&format!("sh {}", shell_quote(&script_path)))?;

	Ok(())
}
