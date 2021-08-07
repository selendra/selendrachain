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
use beefy_primitives::crypto::AuthorityId as BeefyId;
use grandpa::AuthorityId as GrandpaId;
use selendra_runtime as selendra;
use selendra_runtime::constants::currency::UNITS as SEL;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::Forcing;
use selendra_primitives::v1::{AccountId, AccountPublic, AssignmentId, ValidatorId};

use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;
use std::collections::BTreeMap;

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
}

/// The `ChainSpec` parameterized for the selendra runtime.
pub type SelendraChainSpec = service::GenericChainSpec<selendra::GenesisConfig, Extensions>;

pub fn selendra_config() -> Result<SelendraChainSpec, String> {
	SelendraChainSpec::from_json_bytes(&include_bytes!("../res/selendra.json")[..])
}

/// The default parachains host configuration.
fn default_parachains_host_configuration() ->
	selendra_runtime_parachains::configuration::HostConfiguration<selendra_primitives::v1::BlockNumber>
{
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
		max_downward_message_size: 1024,
		// this is approximatelly 4ms.
		//
		// Same as `4 * frame_support::weights::WEIGHT_PER_MILLIS`. We don't bother with
		// an import since that's a made up number and should be replaced with a constant
		// obtained by benchmarking anyway.
		ump_service_total_weight: 4 * 1_000_000_000,
		max_upward_message_size: 1024 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_open_request_ttl: 5,
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
			// 5HC5kafMxWYjXbE6ynh4RhoaouH9p5MetZ29VCsRFL1MpKt1
			hex!["e2cd9722efce97d8f58c90aa36a260885487105b30f886595411ded8149e9d6a"].into(),
			// 5FLbYZF48DjoxfAAPBiHz9nWQbp1sELvKpPoRdeDhYBpp1Ni
			hex!["90d360b84b0fbb18f6c20a0220aa33be50b2c1a523774ce811e8d4f81a851c1f"].into(),
			// 5CDLd7jXEm8u11MzqMPNWenHQGbSKPMREA29XsRNp293Frxw
			hex!["0695bad3b95809621fe5300eab247f81ec3a76058178a39aac1d43df425d4d0a"]
				.unchecked_into(),
			// 5Gpr9h1D7Rf5wfLBQgfR9rLonFWued1ehZH5qwM7BnrSaTiB
			hex!["d29ba662a730a6ad2c834942d093af7bf83f0910b00a61a8ca43dcba98a320f5"]
				.unchecked_into(),
			// 5CDLd7jXEm8u11MzqMPNWenHQGbSKPMREA29XsRNp293Frxw
			hex!["0695bad3b95809621fe5300eab247f81ec3a76058178a39aac1d43df425d4d0a"]
				.unchecked_into(),
			// 5CDLd7jXEm8u11MzqMPNWenHQGbSKPMREA29XsRNp293Frxw
			hex!["0695bad3b95809621fe5300eab247f81ec3a76058178a39aac1d43df425d4d0a"]
				.unchecked_into(),
			// 5CDLd7jXEm8u11MzqMPNWenHQGbSKPMREA29XsRNp293Frxw
			hex!["0695bad3b95809621fe5300eab247f81ec3a76058178a39aac1d43df425d4d0a"]
				.unchecked_into(),
			// 5CDLd7jXEm8u11MzqMPNWenHQGbSKPMREA29XsRNp293Frxw
			hex!["0695bad3b95809621fe5300eab247f81ec3a76058178a39aac1d43df425d4d0a"]
				.unchecked_into(),
		),
		(
			// 5DAuhKbHNiY4L3TraxJ2zQ2KRUKgVF5qUKoTnZnY2iRrvZtZ
			hex!["30f78f18042b49b301b9ae0b53fe69f28591fe8eb1bf6e5084d390dc4da6697a"].into(),
			// 5D5GMz9fVCXThwEnLBy7h1bTguSeP6YBVVr3ifEs9nN9Tuqi
			hex!["2ca9a818b785d75f03934ad728e7ac2e5f9d5ef6e5d1dfe8d42900f3d69f8a37"].into(),
			// 5EuwnnxtbdDyBd4keWJjJ7vY8H9PE6BHwWje3tP3qLMMUwZo
			hex!["7e06651e17c4968162f209e686bcb307a7eb432dc6a19f52da77cb79fe956217"]
				.unchecked_into(),
			// 5EFNmpGagFWCBHyj1tqkX9vK62posgPJJhZFyWded3bGh6fj
			hex!["609beccc0b2448883bcdfb982e2032efc6c0808cc57d64c835c3680976df6a24"]
				.unchecked_into(),
			// 5EuwnnxtbdDyBd4keWJjJ7vY8H9PE6BHwWje3tP3qLMMUwZo
			hex!["7e06651e17c4968162f209e686bcb307a7eb432dc6a19f52da77cb79fe956217"]
				.unchecked_into(),
			// 5EuwnnxtbdDyBd4keWJjJ7vY8H9PE6BHwWje3tP3qLMMUwZo
			hex!["7e06651e17c4968162f209e686bcb307a7eb432dc6a19f52da77cb79fe956217"]
				.unchecked_into(),
			// 5EuwnnxtbdDyBd4keWJjJ7vY8H9PE6BHwWje3tP3qLMMUwZo
			hex!["7e06651e17c4968162f209e686bcb307a7eb432dc6a19f52da77cb79fe956217"]
				.unchecked_into(),
			// 5EuwnnxtbdDyBd4keWJjJ7vY8H9PE6BHwWje3tP3qLMMUwZo
			hex!["7e06651e17c4968162f209e686bcb307a7eb432dc6a19f52da77cb79fe956217"]
				.unchecked_into(),
		),
	];

	const ENDOWMENT: u128 = 1570796325 * SEL;
	const STASH: u128 =  31416 * SEL;

	selendra::GenesisConfig {
		system: selendra::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
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
			validator_count: 10,
			minimum_validator_count: 2,
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
			min_nominator_bond: 314 * SEL,
			min_validator_bond: STASH,
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: Default::default(),
		council: selendra::CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		},
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
		parachains_configuration: selendra::ParachainsConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		gilt: Default::default(),
		paras: Default::default(),
		sudo: selendra::SudoConfig {
			key: endowed_accounts[0].clone(),
		},
		evm: selendra::EvmConfig { 
			accounts: BTreeMap::new(),
		},
		ethereum: selendra::EthereumConfig {},
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

	const ENDOWMENT: u128 = 130_899_693 * SEL;
	const STASH: u128 = 31416 * SEL;

	selendra::GenesisConfig {
		system: selendra::SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		indices: selendra::IndicesConfig { indices: vec![] },
		balances: selendra::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.map(|k| (k.clone(), ENDOWMENT))
				.collect(),
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
			min_nominator_bond: 314 * SEL,
			min_validator_bond: STASH,
			..Default::default()
		},
		phragmen_election: Default::default(),
		democracy: selendra::DemocracyConfig::default(),
		council: selendra::CouncilConfig {
			members: vec![],
			phantom: Default::default(),
		},
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
		parachains_configuration: selendra::ParachainsConfigurationConfig {
			config: default_parachains_host_configuration(),
		},
		gilt: Default::default(),
		paras: Default::default(),
		sudo: selendra::SudoConfig {
			key: endowed_accounts[0].clone(),
		},
		evm: selendra::EvmConfig { 
			accounts: BTreeMap::new(),
		},
		ethereum: selendra::EthereumConfig {},
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

/// Selendra local testnet config (multivalidator Alice + Bob)
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
