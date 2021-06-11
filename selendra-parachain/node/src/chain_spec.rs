use cumulus_primitives_core::ParaId;
use selendra_parachain_runtime::{AccountId, AuraId, Signature};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use selendra_parachain_common::Balance;

const SELENDRA_ED: Balance = selendra_parachain_runtime::constants::currency::EXISTENTIAL_DEPOSIT;

// Specialized `ChainSpec` for the normal parachain runtime.
pub type SelendraChainSpec = sc_service::GenericChainSpec<selendra_parachain_runtime::GenesisConfig, Extensions>;

/// Helper function to generate a crypto pair from seed
pub fn get_pair_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
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
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_pair_from_seed::<TPublic>(seed)).into_account()
}

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_pair_from_seed::<AuraId>(seed)
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn selendra_session_keys(keys: AuraId) -> selendra_parachain_runtime::opaque::SessionKeys {
	selendra_parachain_runtime::opaque::SessionKeys { aura: keys }
}

fn selendra_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> selendra_parachain_runtime::GenesisConfig {

	selendra_parachain_runtime::GenesisConfig {
		system: selendra_parachain_runtime::SystemConfig {
			code: selendra_parachain_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: selendra_parachain_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, SELENDRA_ED))
				.collect(),
		},
		parachain_info: selendra_parachain_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: selendra_parachain_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: SELENDRA_ED,
			..Default::default()
		},
		session: selendra_parachain_runtime::SessionConfig {
			keys: invulnerables.iter().cloned().map(|(acc, aura)| (
				acc.clone(), // account id
				acc.clone(), // validator id
				selendra_session_keys(aura), // session keys
			)).collect()
		},
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
	}
}

pub fn selendra_local_config(id: ParaId) -> SelendraChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "SEL".into());
	properties.insert("tokenDecimals".into(), 18.into());

	SelendraChainSpec::from_genesis(
		// Name
		"Selendra Parachain Local",
		// ID
		"selendra_parachain_local",
		ChainType::Local,
		move || {
			selendra_genesis(
				// initial collators.
				vec![(
						 get_account_id_from_seed::<sr25519::Public>("Alice"),
						 get_collator_keys_from_seed("Alice")
					 ),
					 (
						 get_account_id_from_seed::<sr25519::Public>("Bob"),
						 get_collator_keys_from_seed("Bob")
					 ),
				],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
				],
				id,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "selendra-local".into(),
			para_id: id.into(),
		},
	)
}

pub fn selendra_development_config(id: ParaId) -> SelendraChainSpec {
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "SEL".into());
	properties.insert("tokenDecimals".into(), 18.into());

	SelendraChainSpec::from_genesis(
		// Name
		"Selendra Parachain Development",
		// ID
		"selendra_parachain_dev",
		ChainType::Local,
		move || {
			selendra_genesis(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed("Alice"),
					)
				],
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
				],
				id,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "selendra-dev".into(),
			para_id: id.into(),
		},
	)
}
