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

use beefy_primitives::crypto::AuthorityId as BeefyId;
use grandpa::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use selendra_primitives::v1::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use selendra_runtime as selendra;
use selendra_runtime::constants::currency::UNITS as SEL;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;

use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;

const SELENDRA_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROTOCOL_ID: &str = "sel";

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
	/// The light sync state.
	///
	/// This value will be set by the `sync-state rpc` implementation.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

pub type SelendraChainSpec = service::GenericChainSpec<selendra::GenesisConfig, Extensions>;

pub fn selendra_config() -> Result<SelendraChainSpec, String> {
	SelendraChainSpec::from_json_bytes(&include_bytes!("../res/selendra.json")[..])
}

pub fn selendra_testnet_config() -> Result<SelendraChainSpec, String> {
	SelendraChainSpec::from_json_bytes(&include_bytes!("../res/selendra-testnet.json")[..])
}

fn default_parachains_host_configuration(
) -> selendra_runtime_parachains::configuration::HostConfiguration<
	selendra_primitives::v1::BlockNumber,
> {
	use selendra_primitives::v1::{MAX_CODE_SIZE, MAX_POV_SIZE};

	selendra_runtime_parachains::configuration::HostConfiguration {
		validation_upgrade_frequency: 1u32,
		validation_upgrade_delay: 1,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		group_rotation_frequency: 20,
		chain_availability_period: 4,
		thread_availability_period: 4,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024 * 1024,
		ump_service_total_weight: 100_000_000_000,
		max_upward_message_size: 1024 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_max_parathread_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_parathread_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		..Default::default()
	}
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
	use hex_literal::hex;
	use sp_core::crypto::UncheckedInto;

	// subkey inspect "$SECRET"
	let endowed_accounts = vec![
		// 5CDkUQaKd39SJq9LaUyK8QXbqCTDVgCiDCSu6izYe2pkumBx
		hex!["06e603f736d04565b4fbb38074c0f52a7687c68ffaf58d1438f47cd6de0d397b"].into(),
	];

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			// 5Ggb7sQMr59VuPBswCBvdrUog5qKs6X5tfHfG7vvNNH7RHrX
			hex!["cc4f1627136cc9ab173ca1079fcebcb4b1b374cee0d62ebe2ccbca355e3eaf74"].into(),
			// 5ENSsELpir3F6EzFtqmNa8aiLsvsXe5Kf2qh3XJVLSMi8BAd
			hex!["66006c1c52a614ddd82fac2241bf5f61e9edd4647b114dfe66842104fd40b56c"].into(),
			//  5HH6CY44d9oy3q3WstHAHzazS64ShHuW4yR9kf7qYrWA3yFv
			hex!["e69f52e5d2ec0d361c52262e480e008e41a7d9c1530899edda2bc279b9722d1b"]
				.unchecked_into(),
			// 5HfwyRNJVaTyw7sXTjGFm6qkzAhBmAGHnPEHeJKsxxKxG4Yp
			hex!["f80e46c9954a3d5a93fc65b28efe343fa21699c929d8daf0c625d7a3349d346f"]
				.unchecked_into(),
			//  5HH6CY44d9oy3q3WstHAHzazS64ShHuW4yR9kf7qYrWA3yFv
			hex!["e69f52e5d2ec0d361c52262e480e008e41a7d9c1530899edda2bc279b9722d1b"]
				.unchecked_into(),
			//  5HH6CY44d9oy3q3WstHAHzazS64ShHuW4yR9kf7qYrWA3yFv
			hex!["e69f52e5d2ec0d361c52262e480e008e41a7d9c1530899edda2bc279b9722d1b"]
				.unchecked_into(),
			//  5HH6CY44d9oy3q3WstHAHzazS64ShHuW4yR9kf7qYrWA3yFv
			hex!["e69f52e5d2ec0d361c52262e480e008e41a7d9c1530899edda2bc279b9722d1b"]
				.unchecked_into(),
			//  5HH6CY44d9oy3q3WstHAHzazS64ShHuW4yR9kf7qYrWA3yFv
			hex!["e69f52e5d2ec0d361c52262e480e008e41a7d9c1530899edda2bc279b9722d1b"]
				.unchecked_into(),
		),
		(
			// 5HpXWc9SeeScAKZEdHQqbWqybKSwB2feoEAxpDo47uQqr7jJ
			hex!["fe99233e10d00fb26b37f2c85e3690c8b9998b51f07f3c0bf519b8320a38803a"].into(),
			// 5H41ejd4tDX1XnJCYxw6eXazDMjn2ZHqgouDZH3EvQpCP7sD
			hex!["dca5d29a84f0aaa2bc623f9f11d7f21cafa3b2e358bee3a950ae121fd58b8f31"].into(),
			//  5DhQdjF1xC5gbpWKQ2YyZN6Kuzb62ESYjfqHBkNjwyJS7shd
			hex!["483a55d64f94e36d05f90471bd9857f6e2f58ef53e2b3970ca8e3093a68e6b77"]
				.unchecked_into(),
			// 5C5aNM5GCCEYnUHzPG28UEbDFWybYbbbRGM4eYcpAqcVE1f6
			hex!["00aa099a837c044e9aaef4d40d4cb66efa20bb9da7ecdb3af8afcc4682cc51d2"]
				.unchecked_into(),
			//  5DhQdjF1xC5gbpWKQ2YyZN6Kuzb62ESYjfqHBkNjwyJS7shd
			hex!["483a55d64f94e36d05f90471bd9857f6e2f58ef53e2b3970ca8e3093a68e6b77"]
				.unchecked_into(),
			//  5DhQdjF1xC5gbpWKQ2YyZN6Kuzb62ESYjfqHBkNjwyJS7shd
			hex!["483a55d64f94e36d05f90471bd9857f6e2f58ef53e2b3970ca8e3093a68e6b77"]
				.unchecked_into(),
			//  5DhQdjF1xC5gbpWKQ2YyZN6Kuzb62ESYjfqHBkNjwyJS7shd
			hex!["483a55d64f94e36d05f90471bd9857f6e2f58ef53e2b3970ca8e3093a68e6b77"]
				.unchecked_into(),
			//  5DhQdjF1xC5gbpWKQ2YyZN6Kuzb62ESYjfqHBkNjwyJS7shd
			hex!["483a55d64f94e36d05f90471bd9857f6e2f58ef53e2b3970ca8e3093a68e6b77"]
				.unchecked_into(),
		),
		(
			// 5GKEfy1ZichDYi4uHyx9C6145Z9iVYsQM4zZ7uevsiJ7LguS
			hex!["bc061a86ddb82e9079feb48da3abc67da083b84455713887e83f1426de043b6e"].into(),
			// 5DS1Ut8fsQ92p2EwzAhiDakBvCtQemh2mgKEubfbHwXUF1yY
			hex!["3c7bb743843eea4d4fdb2621bcc2d66fb1b2844fc1fd8ad0ed8faf1840894e26"].into(),
			//  5EHVLhGZ6By7SKUkwuZAhjmrJjvdfbWpvhcDyihSte8cztxd
			hex!["6238859b4e6d3f5f753664d05c8712fcf28f9cebf028a2cabd4d8c21b97d650b"]
				.unchecked_into(),
			// 5F8Jd8WT9mpnLcaqUUsQmYfD6dXRozZHBNQFnFrEfAq52MM8
			hex!["877378a776f65305a4f4016dd72c94d45e298cd93032b6a5fbc6ffc5fa0df324"]
				.unchecked_into(),
			//  5EHVLhGZ6By7SKUkwuZAhjmrJjvdfbWpvhcDyihSte8cztxd
			hex!["6238859b4e6d3f5f753664d05c8712fcf28f9cebf028a2cabd4d8c21b97d650b"]
				.unchecked_into(),
			//  5EHVLhGZ6By7SKUkwuZAhjmrJjvdfbWpvhcDyihSte8cztxd
			hex!["6238859b4e6d3f5f753664d05c8712fcf28f9cebf028a2cabd4d8c21b97d650b"]
				.unchecked_into(),
			//  5EHVLhGZ6By7SKUkwuZAhjmrJjvdfbWpvhcDyihSte8cztxd
			hex!["6238859b4e6d3f5f753664d05c8712fcf28f9cebf028a2cabd4d8c21b97d650b"]
				.unchecked_into(),
			//  5EHVLhGZ6By7SKUkwuZAhjmrJjvdfbWpvhcDyihSte8cztxd
			hex!["6238859b4e6d3f5f753664d05c8712fcf28f9cebf028a2cabd4d8c21b97d650b"]
				.unchecked_into(),
		),
		(
			// 5Gb8Ji9JBTwgQ254iYQmYtKkPBLtREVQFZSJXjJnxu9itHcg
			hex!["c824993c8b7bbd6956b2fb4e7a884faa82b58699008aa9dc5708e7086798410b"].into(),
			// 5GCCQuy5vFBqMBRjSMzLQebZUJMTSJGjdL3vxNw6PmQHg2P4
			hex!["b6a7c79097599d9e870709c82407bc18727ad210f3be0be875c439ae88194e05"].into(),
			//  5F6mhYSaBMsHbAYAEJT9JzdH1Zv8t4DPerB2nCAr5cYYtu7Q
			hex!["8648206fda27ea7f28701be9fe2e02e6bd8357ee31152665a9f3e9a0e7d8cc70"]
				.unchecked_into(),
			// 5G5JmNMtCVBo4G2TnssH6evABFv5VZSzKxAgPeVUgrmzHzLY
			hex!["b166725c8460d13b5d1d72a702c2b974c1b943fb582bc4576455c56fc324585f"]
				.unchecked_into(),
			//  5F6mhYSaBMsHbAYAEJT9JzdH1Zv8t4DPerB2nCAr5cYYtu7Q
			hex!["8648206fda27ea7f28701be9fe2e02e6bd8357ee31152665a9f3e9a0e7d8cc70"]
				.unchecked_into(),
			//  5F6mhYSaBMsHbAYAEJT9JzdH1Zv8t4DPerB2nCAr5cYYtu7Q
			hex!["8648206fda27ea7f28701be9fe2e02e6bd8357ee31152665a9f3e9a0e7d8cc70"]
				.unchecked_into(),
			//  5F6mhYSaBMsHbAYAEJT9JzdH1Zv8t4DPerB2nCAr5cYYtu7Q
			hex!["8648206fda27ea7f28701be9fe2e02e6bd8357ee31152665a9f3e9a0e7d8cc70"]
				.unchecked_into(),
			//  5F6mhYSaBMsHbAYAEJT9JzdH1Zv8t4DPerB2nCAr5cYYtu7Q
			hex!["8648206fda27ea7f28701be9fe2e02e6bd8357ee31152665a9f3e9a0e7d8cc70"]
				.unchecked_into(),
		),
	];

	const ENDOWMENT: u128 = 1570796325 * SEL;
	const STASH: u128 = 31416 * SEL;

	selendra::GenesisConfig {
		system: selendra::SystemConfig { code: wasm_binary.to_vec() },
		balances: selendra::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k: &AccountId| (k.clone(), ENDOWMENT))
				.chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
				.collect(),
		},
		indices: selendra::IndicesConfig { indices: vec![] },
		session: selendra::SessionConfig {
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
		staking: selendra::StakingConfig {
			validator_count: 4,
			minimum_validator_count: 4,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, selendra::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			min_nominator_bond: 314 * SEL,
			min_validator_bond: STASH,
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: selendra::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: selendra::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: selendra::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(selendra::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: selendra::AuthorityDiscoveryConfig { keys: vec![] },
		vesting: selendra::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		configuration: selendra::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: selendra::XcmPalletConfig { safe_xcm_version: Some(2) },
		sudo: selendra::SudoConfig { key: endowed_accounts[0].clone() },
	}
}

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
	(keys.0, keys.1, keys.2, keys.3, keys.4, keys.5, keys.6, keys.7, get_from_seed::<BeefyId>(seed))
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

	const ENDOWMENT: u128 = 500000 * SEL;
	const STASH: u128 = 31416 * SEL;

	selendra::GenesisConfig {
		system: selendra::SystemConfig { code: wasm_binary.to_vec() },
		indices: selendra::IndicesConfig { indices: vec![] },
		balances: selendra::BalancesConfig {
			balances: endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect(),
		},
		session: selendra::SessionConfig {
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
		staking: selendra::StakingConfig {
			minimum_validator_count: 1,
			validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, selendra::StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			min_nominator_bond: 314 * SEL,
			min_validator_bond: STASH,
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: selendra::DemocracyConfig::default(),
		council: selendra::CouncilConfig { members: vec![], phantom: Default::default() },
		technical_committee: selendra::TechnicalCommitteeConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_membership: Default::default(),
		babe: selendra::BabeConfig {
			authorities: Default::default(),
			epoch_config: Some(selendra::BABE_GENESIS_EPOCH_CONFIG),
		},
		grandpa: Default::default(),
		im_online: Default::default(),
		authority_discovery: selendra::AuthorityDiscoveryConfig { keys: vec![] },
		vesting: selendra::VestingConfig { vesting: vec![] },
		treasury: Default::default(),
		configuration: selendra::ConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		paras: Default::default(),
		xcm_pallet: selendra::XcmPalletConfig { safe_xcm_version: Some(2) },
		sudo: selendra::SudoConfig { key: endowed_accounts[0].clone() },
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

pub fn selendra_development_config() -> Result<SelendraChainSpec, String> {
	let wasm_binary = selendra::WASM_BINARY.ok_or("Selendra development wasm not available")?;

	Ok(SelendraChainSpec::from_genesis(
		"Development",
		"selendra_dev",
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

pub fn selendra_local_testnet_config() -> Result<SelendraChainSpec, String> {
	let wasm_binary = selendra::WASM_BINARY.ok_or("Selendra development wasm not available")?;

	Ok(SelendraChainSpec::from_genesis(
		"Selendra Local Testnet",
		"selendra_local_testnet",
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
