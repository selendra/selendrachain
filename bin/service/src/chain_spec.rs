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

//! Indracore chain configurations.

use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use babe_primitives::AuthorityId as BabeId;
use beefy_primitives::ecdsa::AuthorityId as BeefyId;
use grandpa::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::Forcing;
use indracore::constants::currency::SELS;
use indracore_primitives::v1::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use indracore_runtime as indracore;
use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;

const INDRACORE_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "dot";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<indracore_primitives::v1::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<indracore_primitives::v1::Block>,
}

/// The `ChainSpec` parametrised for the indracore runtime.
pub type IndracoreChainSpec = service::GenericChainSpec<indracore::GenesisConfig, Extensions>;

pub fn indracore_config() -> Result<IndracoreChainSpec, String> {
	IndracoreChainSpec::from_json_bytes(&include_bytes!("../res/indracore.json")[..])
}

fn indracore_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	im_online: ImOnlineId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> indracore::SessionKeys {
	indracore::SessionKeys {
		babe,
		grandpa,
		im_online,
		para_validator,
		para_assignment,
		authority_discovery,
	}
}

fn indracore_staging_testnet_config_genesis(wasm_binary: &[u8]) -> indracore::GenesisConfig {
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

	indracore::GenesisConfig {
		frame_system: indracore::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: indracore::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		pallet_indices: indracore::IndicesConfig { indices: vec![] },
		pallet_session: indracore::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						indracore_session_keys(
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
		pallet_staking: indracore::StakingConfig {
			validator_count: 50,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STASH,
						indracore::StakerStatus::Validator,
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
		pallet_collective_Instance1: indracore::CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: indracore::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_membership_Instance1: Default::default(),
		pallet_babe: indracore::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(indracore::BABE_GENESIS_EPOCH_CONFIG),
		},
		pallet_grandpa: Default::default(),
		pallet_im_online: Default::default(),
		pallet_authority_discovery: indracore::AuthorityDiscoveryConfig { keys: vec![] },
		pallet_vesting: indracore::VestingConfig { vesting: vec![] },
		pallet_treasury: Default::default(),
		parachains_configuration: Default::default(),
	}
}

/// Indracore staging testnet config.
pub fn indracore_staging_testnet_config() -> Result<IndracoreChainSpec, String> {
	let wasm_binary = indracore::WASM_BINARY.ok_or("Indracore development wasm not available")?;
	let boot_nodes = vec![];

	Ok(IndracoreChainSpec::from_genesis(
		"Indracore Staging Testnet",
		"indracore_staging_testnet",
		ChainType::Live,
		move || indracore_staging_testnet_config_genesis(wasm_binary),
		boot_nodes,
		Some(
			TelemetryEndpoints::new(vec![(INDRACORE_STAGING_TELEMETRY_URL.to_string(), 0)])
				.expect("Indracore Staging telemetry url is valid; qed"),
		),
		Some(DEFAULT_PROTOCOL_ID),
		None,
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
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
	]
}

/// Helper function to create indracore GenesisConfig for testing
pub fn indracore_testnet_genesis(
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
) -> indracore::GenesisConfig {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * SELS;
	const STASH: u128 = 100 * SELS;

	indracore::GenesisConfig {
		frame_system: indracore::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_indices: indracore::IndicesConfig { indices: vec![] },
		pallet_balances: indracore::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k| (k.clone(), ENDOWMENT))
				.collect(),
		},
		pallet_session: indracore::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						indracore_session_keys(
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
		pallet_staking: indracore::StakingConfig {
			minimum_validator_count: 1,
			validator_count: 2,
			stakers: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.1.clone(),
						STASH,
						indracore::StakerStatus::Validator,
					)
				})
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::NotForcing,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		pallet_elections_phragmen: Default::default(),
		pallet_democracy: indracore::DemocracyConfig::default(),
		pallet_collective_Instance1: indracore::CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: indracore::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_membership_Instance1: Default::default(),
		pallet_babe: indracore::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(indracore::BABE_GENESIS_EPOCH_CONFIG),
		},
		pallet_grandpa: Default::default(),
		pallet_im_online: Default::default(),
		pallet_authority_discovery: indracore::AuthorityDiscoveryConfig { keys: vec![] },
		pallet_vesting: indracore::VestingConfig { vesting: vec![] },
		pallet_treasury: Default::default(),
		parachains_configuration: Default::default(),
	}
}

fn indracore_development_config_genesis(wasm_binary: &[u8]) -> indracore::GenesisConfig {
	indracore_testnet_genesis(
		wasm_binary,
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Indracore development config (single validator Alice)
pub fn indracore_development_config() -> Result<IndracoreChainSpec, String> {
	let wasm_binary = indracore::WASM_BINARY.ok_or("Indracore development wasm not available")?;

	Ok(IndracoreChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		move || indracore_development_config_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Default::default(),
	))
}

fn indracore_local_testnet_genesis(wasm_binary: &[u8]) -> indracore::GenesisConfig {
	indracore_testnet_genesis(
		wasm_binary,
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Indracore local testnet config (multivalidator Alice + Bob)
pub fn indracore_local_testnet_config() -> Result<IndracoreChainSpec, String> {
	let wasm_binary = indracore::WASM_BINARY.ok_or("Indracore development wasm not available")?;

	Ok(IndracoreChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		ChainType::Local,
		move || indracore_local_testnet_genesis(wasm_binary),
		vec![],
		None,
		Some(DEFAULT_PROTOCOL_ID),
		None,
		Default::default(),
	))
}