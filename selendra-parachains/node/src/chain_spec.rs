// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use parachains_common::{AccountId, AuraId, Signature};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<indracore_runtime::GenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

use parachains_common::Balance as IndracoreBalance;

pub type IndracoreChainSpec =
	sc_service::GenericChainSpec<indracore_runtime::GenesisConfig, Extensions>;
pub type IndranetChainSpec =
	sc_service::GenericChainSpec<indranet_runtime::GenesisConfig, Extensions>;

const INDRACORE_ED: IndracoreBalance = indracore_runtime::constants::currency::EXISTENTIAL_DEPOSIT;
const INDRANET_ED: IndracoreBalance = indranet_runtime::constants::currency::EXISTENTIAL_DEPOSIT;

/// Helper function to generate a crypto pair from seed
pub fn get_public_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed<AuraId: Public>(seed: &str) -> <AuraId::Pair as Pair>::Public {
	get_public_from_seed::<AuraId>(seed)
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn indracore_session_keys(keys: AuraId) -> indracore_runtime::SessionKeys {
	indracore_runtime::SessionKeys { aura: keys }
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn indranet_session_keys(keys: AuraId) -> indranet_runtime::SessionKeys {
	indranet_runtime::SessionKeys { aura: keys }
}

pub fn indracore_development_config() -> IndracoreChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 972.into());
	properties.insert("tokenSymbol".into(), "SEL".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndracoreChainSpec::from_genesis(
		// Name
		"Indracore Development",
		// ID
		"indracore_dev",
		ChainType::Local,
		move || {
			indracore_genesis(
				// initial collators.
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_collator_keys_from_seed::<AuraId>("Alice"),
				)],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
				],
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "selendra-dev".into(), para_id: 1000 },
	)
}

pub fn indracore_local_config() -> IndracoreChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 972.into());
	properties.insert("tokenSymbol".into(), "SEL".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndracoreChainSpec::from_genesis(
		// Name
		"Indracore Local",
		// ID
		"indracore_local",
		ChainType::Local,
		move || {
			indracore_genesis(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_collator_keys_from_seed::<AuraId>("Bob"),
					),
				],
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
				],
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "selendra-local".into(), para_id: 1000 },
	)
}

pub fn indracore_config() -> IndracoreChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("ss58Format".into(), 972.into());
	properties.insert("tokenSymbol".into(), "SEL".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndracoreChainSpec::from_genesis(
		// Name
		"Indracore",
		// ID
		"indracore",
		ChainType::Live,
		move || {
			indracore_genesis(
				// initial collators.
				vec![
					(
						hex!("34b9c5ac9c081aa4a4f2325e9fcf903176ed44e418d378e68675ada9819a9507")
							.into(),
						hex!("34b9c5ac9c081aa4a4f2325e9fcf903176ed44e418d378e68675ada9819a9507")
							.unchecked_into(),
					),
					(
						hex!("9a04bb9cd411a5b0187adb4a3d048783c192fba85695841c68c4cd3627e5a103")
							.into(),
						hex!("9a04bb9cd411a5b0187adb4a3d048783c192fba85695841c68c4cd3627e5a103")
							.unchecked_into(),
					),
					(
						hex!("7e8dac940fcd738e7e83f25465ae09ff73abc1682740edd159cd9c1777887a77")
							.into(),
						hex!("7e8dac940fcd738e7e83f25465ae09ff73abc1682740edd159cd9c1777887a77")
							.unchecked_into(),
					),
					(
						hex!("66af9950bff0dd612f5905022c4cd45501d23b2a6422924501e5592fbd839510")
							.into(),
						hex!("66af9950bff0dd612f5905022c4cd45501d23b2a6422924501e5592fbd839510")
							.unchecked_into(),
					),
				],
				Vec::new(),
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "selendra".into(), para_id: 1000 },
	)
}

fn indracore_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> indracore_runtime::GenesisConfig {
	indracore_runtime::GenesisConfig {
		system: indracore_runtime::SystemConfig {
			code: indracore_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: indracore_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, INDRACORE_ED))
				.collect(),
		},
		parachain_info: indracore_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: indracore_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: INDRACORE_ED * 16,
			..Default::default()
		},
		session: indracore_runtime::SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                  // account id
						acc,                          // validator id
						indracore_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		selendra_xcm: indracore_runtime::SelendraXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
	}
}

pub fn indranet_development_config() -> IndranetChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "CDM".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndranetChainSpec::from_genesis(
		// Name
		"Indranet Development",
		// ID
		"indranet_dev",
		ChainType::Local,
		move || {
			indranet_genesis(
				// initial collators.
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_collator_keys_from_seed::<AuraId>("Alice"),
				)],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
				],
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "cardamom-dev".into(), para_id: 1000 },
	)
}

pub fn indranet_local_config() -> IndranetChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "CDM".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndranetChainSpec::from_genesis(
		// Name
		"Indranet Local",
		// ID
		"indranet_local",
		ChainType::Local,
		move || {
			indranet_genesis(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_collator_keys_from_seed::<AuraId>("Bob"),
					),
				],
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
				],
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "cardamom-local".into(), para_id: 1000 },
	)
}

pub fn indranet_config() -> IndranetChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "CDM".into());
	properties.insert("tokenDecimals".into(), 18.into());

	IndranetChainSpec::from_genesis(
		// Name
		"Indranet",
		// ID
		"indranet",
		ChainType::Live,
		move || {
			indranet_genesis(
				// initial collators.
				vec![
					(
						hex!("cc4f1627136cc9ab173ca1079fcebcb4b1b374cee0d62ebe2ccbca355e3eaf74")
							.into(),
						hex!("cc4f1627136cc9ab173ca1079fcebcb4b1b374cee0d62ebe2ccbca355e3eaf74")
							.unchecked_into(),
					),
					(
						hex!("fe99233e10d00fb26b37f2c85e3690c8b9998b51f07f3c0bf519b8320a38803a")
							.into(),
						hex!("fe99233e10d00fb26b37f2c85e3690c8b9998b51f07f3c0bf519b8320a38803a")
							.unchecked_into(),
					),
					(
						hex!("bc061a86ddb82e9079feb48da3abc67da083b84455713887e83f1426de043b6e")
							.into(),
						hex!("bc061a86ddb82e9079feb48da3abc67da083b84455713887e83f1426de043b6e")
							.unchecked_into(),
					),
					(
						hex!("77b08b89c98682e9478f2638247b35896cb6cdf107852c429ef57e6da0a5b4ae")
							.into(),
						hex!("77b08b89c98682e9478f2638247b35896cb6cdf107852c429ef57e6da0a5b4ae")
							.unchecked_into(),
					),
				],
				Vec::new(),
				1000.into(),
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions { relay_chain: "cardamom".into(), para_id: 1000 },
	)
}

fn indranet_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> indranet_runtime::GenesisConfig {
	let root_key = hex!("77b08b89c98682e9478f2638247b35896cb6cdf107852c429ef57e6da0a5b4ae").into();
	indranet_runtime::GenesisConfig {
		system: indranet_runtime::SystemConfig {
			code: indranet_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: indranet_runtime::BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, INDRANET_ED)).collect(),
		},
		parachain_info: indranet_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: indranet_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: INDRANET_ED * 16,
			..Default::default()
		},
		session: indranet_runtime::SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                 // account id
						acc,                         // validator id
						indranet_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		selendra_xcm: indranet_runtime::SelendraXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
		sudo: indranet_runtime::SudoConfig { key: Some(root_key) },
	}
}
