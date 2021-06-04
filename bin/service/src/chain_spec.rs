// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Selendra chain configurations.

use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use beefy_primitives::ecdsa::AuthorityId as BeefyId;
use grandpa::AuthorityId as GrandpaId;
use selendra_runtime as selendra;
use selendra_runtime::constants::currency::SELS as SELS;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::Forcing;
use selendra_primitives::v1::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;

const SELENDRA_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "dot";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<selendra_primitives::v1::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<selendra_primitives::v1::Block>,
}

/// The `ChainSpec` parametrised for the selendra runtime.
pub type SelendraChainSpec = service::GenericChainSpec<selendra::GenesisConfig, Extensions>;

pub fn selendra_config() -> Result<SelendraChainSpec, String> {
	SelendraChainSpec::from_json_bytes(&include_bytes!("../res/selendra.json")[..])
}

fn selendra_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> selendra::SessionKeys {
	selendra::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

fn selendra_staging_testnet_config_genesis(wasm_binary: &[u8]) -> selendra::GenesisConfig {
	// subkey inspect "$SECRET"
	let endowed_accounts = vec![];

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![];

	const ENDOWMENT: u128 = 1_000_000 * SELS;
	const STASH: u128 = 100 * SELS;

	selendra::GenesisConfig {
		frame_system: selendra::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: selendra::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		pallet_indices: selendra::IndicesConfig { indices: vec![] },
		pallet_session: selendra::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						selendra_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		pallet_staking: selendra::StakingConfig {
			validator_count: 50,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STASH,
						selendra::StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		pallet_elections_phragmen: Default::default(),
		pallet_democracy: Default::default(),
		pallet_collective_Instance1: selendra::CouncilConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		},
		pallet_collective_Instance2: selendra::TechnicalCommitteeConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		},
		pallet_membership_Instance1: Default::default(),
		pallet_babe: selendra::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(selendra::BABE_GENESIS_EPOCH_CONFIG),
		},
		pallet_grandpa: Default::default(),
		pallet_im_online: Default::default(),
		pallet_authority_discovery: selendra::AuthorityDiscoveryConfig { keys: vec![] },
		pallet_vesting: selendra::VestingConfig { vesting: vec![] },
		pallet_treasury: Default::default(),
		pallet_sudo: selendra::SudoConfig {
			key: endowed_accounts[0].clone(),
		},
		parachains_configuration: Default::default(),
		parachains_paras: Default::default(),
	}
}

/// Staging testnet config.
pub fn selendra_staging_testnet_config() -> Result<SelendraChainSpec, String> {
	let wasm_binary = selendra::WASM_BINARY.ok_or("Selendra development wasm not available")?;
	let boot_nodes = vec![];

	Ok(SelendraChainSpec::from_genesis(
		"Selendra Staging Testnet",
		"selendra_staging_testnet",
		ChainType::Live,
		move || selendra_staging_testnet_config_genesis(wasm_binary),
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(SELENDRA_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Selendra Staging telemetry url is valid; qed"),
		),
		Some(DEFAULT_PROTOCOL_ID),
		Some(
            serde_json::from_str(
                "{
            \"tokenDecimals\": 18,
            \"tokenSymbol\": \"SEL\"
        	}",
            )
            .expect("Provided valid json map"),
        ),
		Default::default(),
	))
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ImOnlineId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
	BeefyId,
) {
	let keys = get_authority_keys_from_seed_no_beefy(seed);
	(
		keys.0, keys.1, keys.2, keys.3, keys.4, keys.5, keys.6, keys.7, get_from_seed::<BeefyId>(seed)
	)
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed_no_beefy(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ImOnlineId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<ValidatorId>(seed),
		get_from_seed::<AssignmentId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

fn testnet_accounts() -> Vec<AccountId> {
	vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
	]
}

/// Helper function to create selendra GenesisConfig for testing
pub fn selendra_testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)>,
	_root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> selendra::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * SELS;
	const STASH: u128 = 100 * SELS;

	selendra::GenesisConfig {
		frame_system: selendra::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_indices: selendra::IndicesConfig { indices: vec![] },
		pallet_balances: selendra::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k| (k.clone(), ENDOWMENT))
				.collect(),
		},
		pallet_session: selendra::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						selendra_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		pallet_staking: selendra::StakingConfig {
			minimum_validator_count: 1,
			validator_count: 2,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STASH,
						selendra::StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::NotForcing,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		pallet_elections_phragmen: Default::default(),
		pallet_democracy: selendra::DemocracyConfig::default(),
		pallet_collective_Instance1: selendra::CouncilConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		},
		pallet_collective_Instance2: selendra::TechnicalCommitteeConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		},
		pallet_membership_Instance1: Default::default(),
		pallet_babe: selendra::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(selendra::BABE_GENESIS_EPOCH_CONFIG),
		},
		pallet_grandpa: Default::default(),
		pallet_im_online: Default::default(),
		pallet_authority_discovery: selendra::AuthorityDiscoveryConfig { keys: vec![] },
		pallet_vesting: selendra::VestingConfig { vesting: vec![] },
		pallet_treasury: Default::default(),
		pallet_sudo: selendra::SudoConfig {
			key: endowed_accounts[0].clone(),
		},
		parachains_configuration: Default::default(),
		parachains_paras: Default::default(),
	}
}

fn selendra_development_config_genesis(wasm_binary: &[u8]) -> selendra::GenesisConfig {
	selendra_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Selendra development config (single validator Alice)
pub fn selendra_development_config() -> Result<SelendraChainSpec, String> {
	let wasm_binary = selendra::WASM_BINARY.ok_or("Selendra development wasm not available")?;

	Ok(SelendraChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		move || selendra_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		Some(
            serde_json::from_str(
                "{
            \"tokenDecimals\": 18,
            \"tokenSymbol\": \"SEL\"
        	}",
            )
            .expect("Provided valid json map"),
        ),
		Default::default(),
	))
}

fn selendra_local_testnet_genesis(wasm_binary: &[u8]) -> selendra::GenesisConfig {
	selendra_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Selendra local testnet config (multivalidator Alice + Bob)
pub fn selendra_local_testnet_config() -> Result<SelendraChainSpec, String> {
	let wasm_binary = selendra::WASM_BINARY.ok_or("Selendra development wasm not available")?;

	Ok(SelendraChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		ChainType::Local,
		move || selendra_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		Some(
            serde_json::from_str(
                "{
            \"tokenDecimals\": 18,
            \"tokenSymbol\": \"SEL\"
        	}",
            )
            .expect("Provided valid json map"),
        ),
		Default::default(),
	))
}