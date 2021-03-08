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

use babe_primitives::AuthorityId as BabeId;
use grandpa::AuthorityId as GrandpaId;
use hex_literal::hex;
use indracore::constants::currency::SELS;
use indracore_primitives::v1::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use indracore_runtime as indracore;
use kumandra_runtime as kumandra;
use kumandra_runtime::constants::currency::SELS as REL;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_staking::Forcing;
use sc_chain_spec::{ChainSpecExtension, ChainType};
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::{traits::IdentifyAccount, Perbill};
use telemetry::TelemetryEndpoints;

const INDRACORE_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const KUMANDRA_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
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

/// The `ChainSpec` parametrized for the kumandra runtime.
pub type KumandraChainSpec = service::GenericChainSpec<KumandraGenesisExt, Extensions>;

/// Extension for the Kumandra genesis config to support a custom changes to the genesis state.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct KumandraGenesisExt {
    /// The runtime genesis config.
    runtime_genesis_config: kumandra::GenesisConfig,
    /// The session length in blocks.
    ///
    /// If `None` is supplied, the default value is used.
    session_length_in_blocks: Option<u32>,
}

impl sp_runtime::BuildStorage for KumandraGenesisExt {
    fn assimilate_storage(&self, storage: &mut sp_core::storage::Storage) -> Result<(), String> {
        sp_state_machine::BasicExternalities::execute_with_storage(storage, || {
            if let Some(length) = self.session_length_in_blocks.as_ref() {
                kumandra::constants::time::EpochDurationInBlocks::set(length);
            }
        });
        self.runtime_genesis_config.assimilate_storage(storage)
    }
}

pub fn indracore_config() -> Result<IndracoreChainSpec, String> {
    IndracoreChainSpec::from_json_bytes(&include_bytes!("../res/indracore-sel.json")[..])
}

pub fn kumandra_config() -> Result<IndracoreChainSpec, String> {
    IndracoreChainSpec::from_json_bytes(&include_bytes!("../res/kumandra.json")[..])
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

fn kumandra_session_keys(
    babe: BabeId,
    grandpa: GrandpaId,
    im_online: ImOnlineId,
    para_validator: ValidatorId,
    para_assignment: AssignmentId,
    authority_discovery: AuthorityDiscoveryId,
) -> kumandra_runtime::SessionKeys {
    kumandra_runtime::SessionKeys {
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
        frame_system: Some(indracore::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_balances: Some(indracore::BalancesConfig {
            balances: endowed_accounts
                .iter()
                .map(|k: &AccountId| (k.clone(), ENDOWMENT))
                .chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
                .collect(),
        }),
        pallet_indices: Some(indracore::IndicesConfig { indices: vec![] }),
        pallet_session: Some(indracore::SessionConfig {
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
        }),
        pallet_staking: Some(indracore::StakingConfig {
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
        }),
        pallet_elections_phragmen: Some(Default::default()),
        pallet_democracy: Some(Default::default()),
        pallet_collective_Instance1: Some(indracore::CouncilConfig {
            members: vec![],
            phantom: Default::default(),
        }),
        pallet_collective_Instance2: Some(indracore::TechnicalCommitteeConfig {
            members: vec![],
            phantom: Default::default(),
        }),
        pallet_membership_Instance1: Some(Default::default()),
        pallet_babe: Some(Default::default()),
        pallet_grandpa: Some(Default::default()),
        pallet_im_online: Some(Default::default()),
        pallet_authority_discovery: Some(indracore::AuthorityDiscoveryConfig { keys: vec![] }),
        pallet_vesting: Some(indracore::VestingConfig { vesting: vec![] }),
        pallet_treasury: Some(Default::default()),
        pallet_sudo: Some(indracore::SudoConfig {
            key: endowed_accounts[0].clone(),
        }),
        pallet_contracts: Some(indracore::ContractsConfig {
            current_schedule: pallet_contracts::Schedule {
                ..Default::default()
            },
        }),
    }
}

fn kumandra_staging_testnet_config_genesis(wasm_binary: &[u8]) -> kumandra_runtime::GenesisConfig {
    // subkey inspect "$SECRET"
    let endowed_accounts = vec![
        // 5FeyRQmjtdHoPH56ASFW76AJEP1yaQC1K9aEMvJTF9nzt9S9
        hex!["9ed7705e3c7da027ba0583a22a3212042f7e715d3c168ba14f1424e2bc111d00"].into(),
    ];

    // ./scripts/prepare-test-net.sh 8
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
            //5EHZkbp22djdbuMFH9qt1DVzSCvqi3zWpj6DAYfANa828oei
            hex!["62475fe5406a7cb6a64c51d0af9d3ab5c2151bcae982fb812f7a76b706914d6a"].into(),
            //5FeSEpi9UYYaWwXXb3tV88qtZkmSdB3mvgj3pXkxKyYLGhcd
            hex!["9e6e781a76810fe93187af44c79272c290c2b9e2b8b92ee11466cd79d8023f50"].into(),
            //5Fh6rDpMDhM363o1Z3Y9twtaCPfizGQWCi55BSykTQjGbP7H
            hex!["a076ef1280d768051f21d060623da3ab5b56944d681d303ed2d4bf658c5bed35"]
                .unchecked_into(),
            //5CPd3zoV9Aaah4xWucuDivMHJ2nEEmpdi864nPTiyRZp4t87
            hex!["0e6d7d1afbcc6547b92995a394ba0daed07a2420be08220a5a1336c6731f0bfa"]
                .unchecked_into(),
            //5F7BEa1LGFksUihyatf3dCDYneB8pWzVyavnByCsm5nBgezi
            hex!["86975a37211f8704e947a365b720f7a3e2757988eaa7d0f197e83dba355ef743"]
                .unchecked_into(),
            //5CP6oGfwqbEfML8efqm1tCZsUgRsJztp9L8ZkEUxA16W8PPz
            hex!["0e07a51d3213842f8e9363ce8e444255990a225f87e80a3d651db7841e1a0205"]
                .unchecked_into(),
            //5HQdwiDh8Qtd5dSNWajNYpwDvoyNWWA16Y43aEkCNactFc2b
            hex!["ec60e71fe4a567ef9fef99d4bbf37ffae70564b41aa6f94ef0317c13e0a5477b"]
                .unchecked_into(),
            //5HbSgM72xVuscsopsdeG3sCSCYdAeM1Tay9p79N6ky6vwDGq
            hex!["f49eae66a0ac9f610316906ec8f1a0928e20d7059d76a5ca53cbcb5a9b50dd3c"]
                .unchecked_into(),
        ),
        (
            //5DvH8oEjQPYhzCoQVo7WDU91qmQfLZvxe9wJcrojmJKebCmG
            hex!["520b48452969f6ddf263b664de0adb0c729d0e0ad3b0e5f3cb636c541bc9022a"].into(),
            //5ENZvCRzyXJJYup8bM6yEzb2kQHEb1NDpY2ZEyVGBkCfRdj3
            hex!["6618289af7ae8621981ffab34591e7a6486e12745dfa3fd3b0f7e6a3994c7b5b"].into(),
            //5DLjSUfqZVNAADbwYLgRvHvdzXypiV1DAEaDMjcESKTcqMoM
            hex!["38757d0de00a0c739e7d7984ef4bc01161bd61e198b7c01b618425c16bb5bd5f"]
                .unchecked_into(),
            //5HnDVBN9mD6mXyx8oryhDbJtezwNSj1VRXgLoYCBA6uEkiao
            hex!["fcd5f87a6fd5707a25122a01b4dac0a8482259df7d42a9a096606df1320df08d"]
                .unchecked_into(),
            //5DhyXZiuB1LvqYKFgT5tRpgGsN3is2cM9QxgW7FikvakbAZP
            hex!["48a910c0af90898f11bd57d37ceaea53c78994f8e1833a7ade483c9a84bde055"]
                .unchecked_into(),
            //5EPEWRecy2ApL5n18n3aHyU1956zXTRqaJpzDa9DoqiggNwF
            hex!["669a10892119453e9feb4e3f1ee8e028916cc3240022920ad643846fbdbee816"]
                .unchecked_into(),
            //5ES3fw5X4bndSgLNmtPfSbM2J1kLqApVB2CCLS4CBpM1UxUZ
            hex!["68bf52c482630a8d1511f2edd14f34127a7d7082219cccf7fd4c6ecdb535f80d"]
                .unchecked_into(),
            //5HeXbwb5PxtcRoopPZTp5CQun38atn2UudQ8p2AxR5BzoaXw
            hex!["f6f8fe475130d21165446a02fb1dbce3a7bf36412e5d98f4f0473aed9252f349"]
                .unchecked_into(),
        ),
        (
            //5FPMzsezo1PRxYbVpJMWK7HNbR2kUxidsAAxH4BosHa4wd6S
            hex!["92ef83665b39d7a565e11bf8d18d41d45a8011601c339e57a8ea88c8ff7bba6f"].into(),
            //5G6NQidFG7YiXsvV7hQTLGArir9tsYqD4JDxByhgxKvSKwRx
            hex!["b235f57244230589523271c27b8a490922ffd7dccc83b044feaf22273c1dc735"].into(),
            //5GpZhzAVg7SAtzLvaAC777pjquPEcNy1FbNUAG2nZvhmd6eY
            hex!["d2644c1ab2c63a3ad8d40ad70d4b260969e3abfe6d7e6665f50dc9f6365c9d2a"]
                .unchecked_into(),
            //5HAes2RQYPbYKbLBfKb88f4zoXv6pPA6Ke8CjN7dob3GpmSP
            hex!["e1b68fbd84333e31486c08e6153d9a1415b2e7e71b413702b7d64e9b631184a1"]
                .unchecked_into(),
            //5HTXBf36LXmkFWJLokNUK6fPxVpkr2ToUnB1pvaagdGu4c1T
            hex!["ee93e26259decb89afcf17ef2aa0fa2db2e1042fb8f56ecfb24d19eae8629878"]
                .unchecked_into(),
            //5FtAGDZYJKXkhVhAxCQrXmaP7EE2mGbBMfmKDHjfYDgq2BiU
            hex!["a8e61ffacafaf546283dc92d14d7cc70ea0151a5dd81fdf73ff5a2951f2b6037"]
                .unchecked_into(),
            //5CtK7JHv3h6UQZ44y54skxdwSVBRtuxwPE1FYm7UZVhg8rJV
            hex!["244f3421b310c68646e99cdbf4963e02067601f57756b072a4b19431448c186e"]
                .unchecked_into(),
            //5D4r6YaB6F7A7nvMRHNFNF6zrR9g39bqDJFenrcaFmTCRwfa
            hex!["2c57f81fd311c1ab53813c6817fe67f8947f8d39258252663b3384ab4195494d"]
                .unchecked_into(),
        ),
        (
            //5DMNx7RoX6d7JQ38NEM7DWRcW2THu92LBYZEWvBRhJeqcWgR
            hex!["38f3c2f38f6d47f161e98c697bbe3ca0e47c033460afda0dda314ab4222a0404"].into(),
            //5GGdKNDr9P47dpVnmtq3m8Tvowwf1ot1abw6tPsTYYFoKm2v
            hex!["ba0898c1964196474c0be08d364cdf4e9e1d47088287f5235f70b0590dfe1704"].into(),
            //5EjkyPCzR2SjhDZq8f7ufsw6TfkvgNRepjCRQFc4TcdXdaB1
            hex!["764186bc30fd5a02477f19948dc723d6d57ab174debd4f80ed6038ec960bfe21"]
                .unchecked_into(),
            //5DJV3zCBTJBLGNDCcdWrYxWDacSz84goGTa4pFeKVvehEBte
            hex!["36be9069cdb4a8a07ecd51f257875150f0a8a1be44a10d9d98dabf10a030aef4"]
                .unchecked_into(),
            //5FHf8kpK4fPjEJeYcYon2gAPwEBubRvtwpzkUbhMWSweKPUY
            hex!["8e95b9b5b4dc69790b67b566567ca8bf8cdef3a3a8bb65393c0d1d1c87cd2d2c"]
                .unchecked_into(),
            //5F9FsRjpecP9GonktmtFL3kjqNAMKjHVFjyjRdTPa4hbQRZA
            hex!["882d72965e642677583b333b2d173ac94b5fd6c405c76184bb14293be748a13b"]
                .unchecked_into(),
            //5F1FZWZSj3JyTLs8sRBxU6QWyGLSL9BMRtmSKDmVEoiKFxSP
            hex!["821271c99c958b9220f1771d9f5e29af969edfa865631dba31e1ab7bc0582b75"]
                .unchecked_into(),
            //5CtgRR74VypK4h154s369abs78hDUxZSJqcbWsfXvsjcHJNA
            hex!["2496f28d887d84705c6dae98aee8bf90fc5ad10bb5545eca1de6b68425b70f7c"]
                .unchecked_into(),
        ),
        (
            //5C8AL1Zb4bVazgT3EgDxFgcow1L4SJjVu44XcLC9CrYqFN4N
            hex!["02a2d8cfcf75dda85fafc04ace3bcb73160034ed1964c43098fb1fe831de1b16"].into(),
            //5FLYy3YKsAnooqE4hCudttAsoGKbVG3hYYBtVzwMjJQrevPa
            hex!["90cab33f0bb501727faa8319f0845faef7d31008f178b65054b6629fe531b772"].into(),
            //5Et3tfbVf1ByFThNAuUq5pBssdaPPskip5yob5GNyUFojXC7
            hex!["7c94715e5dd8ab54221b1b6b2bfa5666f593f28a92a18e28052531de1bd80813"]
                .unchecked_into(),
            //5EX1JBghGbQqWohTPU6msR9qZ2nYPhK9r3RTQ2oD1K8TCxaG
            hex!["6c878e33b83c20324238d22240f735457b6fba544b383e70bb62a27b57380c81"]
                .unchecked_into(),
            //5GqL8RbVAuNXpDhjQi1KrS1MyNuKhvus2AbmQwRGjpuGZmFu
            hex!["d2f9d537ffa59919a4028afdb627c14c14c97a1547e13e8e82203d2049b15b1a"]
                .unchecked_into(),
            //5EUNaBpX9mJgcmLQHyG5Pkms6tbDiKuLbeTEJS924Js9cA1N
            hex!["6a8570b9c6408e54bacf123cc2bb1b0f087f9c149147d0005badba63a5a4ac01"]
                .unchecked_into(),
            //5CaZuueRVpMATZG4hkcrgDoF4WGixuz7zu83jeBdY3bgWGaG
            hex!["16c69ea8d595e80b6736f44be1eaeeef2ac9c04a803cc4fd944364cb0d617a33"]
                .unchecked_into(),
            //5DABsdQCDUGuhzVGWe5xXzYQ9rtrVxRygW7RXf9Tsjsw1aGJ
            hex!["306ac5c772fe858942f92b6e28bd82fb7dd8cdd25f9a4626c1b0eee075fcb531"]
                .unchecked_into(),
        ),
        (
            //5C8XbDXdMNKJrZSrQURwVCxdNdk8AzG6xgLggbzuA399bBBF
            hex!["02ea6bfa8b23b92fe4b5db1063a1f9475e3acd0ab61e6b4f454ed6ba00b5f864"].into(),
            //5GsyzFP8qtF8tXPSsjhjxAeU1v7D1PZofuQKN9TdCc7Dp1JM
            hex!["d4ffc4c05b47d1115ad200f7f86e307b20b46c50e1b72a912ec4f6f7db46b616"].into(),
            //5GHWB8ZDzegLcMW7Gdd1BS6WHVwDdStfkkE4G7KjPjZNJBtD
            hex!["bab3cccdcc34401e9b3971b96a662686cf755aa869a5c4b762199ce531b12c5b"]
                .unchecked_into(),
            //5GzDPGbUM9uH52ZEwydasTj8edokGUJ7vEpoFWp9FE1YNuFB
            hex!["d9c056c98ca0e6b4eb7f5c58c007c1db7be0fe1f3776108f797dd4990d1ccc33"]
                .unchecked_into(),
            //5GWZbVkJEfWZ7fRca39YAQeqri2Z7pkeHyd7rUctUHyQifLp
            hex!["c4a980da30939d5bb9e4a734d12bf81259ae286aa21fa4b65405347fa40eff35"]
                .unchecked_into(),
            //5CmLCFeSurRXXtwMmLcVo7sdJ9EqDguvJbuCYDcHkr3cpqyE
            hex!["1efc23c0b51ad609ab670ecf45807e31acbd8e7e5cb7c07cf49ee42992d2867c"]
                .unchecked_into(),
            //5DnsSy8a8pfE2aFjKBDtKw7WM1V4nfE5sLzP15MNTka53GqS
            hex!["4c64d3f06d28adeb36a892fdaccecace150bec891f04694448a60b74fa469c22"]
                .unchecked_into(),
            //5CZdFnyzZvKetZTeUwj5APAYskVJe4QFiTezo5dQNsrnehGd
            hex!["160ea09c5717270e958a3da42673fa011613a9539b2e4ebcad8626bc117ca04a"]
                .unchecked_into(),
        ),
        (
            //5HinEonzr8MywkqedcpsmwpxKje2jqr9miEwuzyFXEBCvVXM
            hex!["fa373e25a1c4fe19c7148acde13bc3db1811cf656dc086820f3dda736b9c4a00"].into(),
            //5EHJbj6Td6ks5HDnyfN4ttTSi57osxcQsQexm7XpazdeqtV7
            hex!["62145d721967bd88622d08625f0f5681463c0f1b8bcd97eb3c2c53f7660fd513"].into(),
            //5EeCsC58XgJ1DFaoYA1WktEpP27jvwGpKdxPMFjicpLeYu96
            hex!["720537e2c1c554654d73b3889c3ef4c3c2f95a65dd3f7c185ebe4afebed78372"]
                .unchecked_into(),
            //5DnEySxbnppWEyN8cCLqvGjAorGdLRg2VmkY96dbJ1LHFK8N
            hex!["4bea0b37e0cce9bddd80835fa2bfd5606f5dcfb8388bbb10b10c483f0856cf14"]
                .unchecked_into(),
            //5E1Y1FJ7dVP7qtE3wm241pTm72rTMcDT5Jd8Czv7Pwp7N3AH
            hex!["560d90ca51e9c9481b8a9810060e04d0708d246714960439f804e5c6f40ca651"]
                .unchecked_into(),
            //5CAC278tFCHAeHYqE51FTWYxHmeLcENSS1RG77EFRTvPZMJT
            hex!["042f07fc5268f13c026bbe199d63e6ac77a0c2a780f71cda05cee5a6f1b3f11f"]
                .unchecked_into(),
            //5HjRTLWcQjZzN3JDvaj1UzjNSayg5ZD9ZGWMstaL7Ab2jjAa
            hex!["fab485e87ed1537d089df521edf983a777c57065a702d7ed2b6a2926f31da74f"]
                .unchecked_into(),
            //5ELv74v7QcsS6FdzvG4vL2NnYDGWmRnJUSMKYwdyJD7Xcdi7
            hex!["64d59feddb3d00316a55906953fb3db8985797472bd2e6c7ea1ab730cc339d7f"]
                .unchecked_into(),
        ),
        (
            //5Ey3NQ3dfabaDc16NUv7wRLsFCMDFJSqZFzKVycAsWuUC6Di
            hex!["8062e9c21f1d92926103119f7e8153cebdb1e5ab3e52d6f395be80bb193eab47"].into(),
            //5HiWsuSBqt8nS9pnggexXuHageUifVPKPHDE2arTKqhTp1dV
            hex!["fa0388fa88f3f0cb43d583e2571fbc0edad57dff3a6fd89775451dd2c2b8ea00"].into(),
            //5H168nKX2Yrfo3bxj7rkcg25326Uv3CCCnKUGK6uHdKMdPt8
            hex!["da6b2df18f0f9001a6dcf1d301b92534fe9b1f3ccfa10c49449fee93adaa8349"]
                .unchecked_into(),
            //5DrA2fZdzmNqT5j6DXNwVxPBjDV9jhkAqvjt6Us3bQHKy3cF
            hex!["4ee66173993dd0db5d628c4c9cb61a27b76611ad3c3925947f0d0011ee2c5dcc"]
                .unchecked_into(),
            //5FNFDUGNLUtqg5LgrwYLNmBiGoP8KRxsvQpBkc7GQP6qaBUG
            hex!["92156f54a114ee191415898f2da013d9db6a5362d6b36330d5fc23e27360ab66"]
                .unchecked_into(),
            //5Gx6YeNhynqn8qkda9QKpc9S7oDr4sBrfAu516d3sPpEt26F
            hex!["d822d4088b20dca29a580a577a97d6f024bb24c9550bebdfd7d2d18e946a1c7d"]
                .unchecked_into(),
            //5DhDcHqwxoes5s89AyudGMjtZXx1nEgrk5P45X88oSTR3iyx
            hex!["481538f8c2c011a76d7d57db11c2789a5e83b0f9680dc6d26211d2f9c021ae4c"]
                .unchecked_into(),
            //5DqAvikdpfRdk5rR35ZobZhqaC5bJXZcEuvzGtexAZP1hU3T
            hex!["4e262811acdfe94528bfc3c65036080426a0e1301b9ada8d687a70ffcae99c26"]
                .unchecked_into(),
        ),
    ];

    const ENDOWMENT: u128 = 1_000_000 * REL;
    const STASH: u128 = 100 * REL;

    kumandra_runtime::GenesisConfig {
        frame_system: Some(kumandra_runtime::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_balances: Some(kumandra_runtime::BalancesConfig {
            balances: endowed_accounts
                .iter()
                .map(|k: &AccountId| (k.clone(), ENDOWMENT))
                .chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
                .collect(),
        }),
        pallet_indices: Some(kumandra_runtime::IndicesConfig { indices: vec![] }),
        pallet_session: Some(kumandra_runtime::SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        kumandra_session_keys(
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
        }),
        pallet_babe: Some(Default::default()),
        pallet_grandpa: Some(Default::default()),
        pallet_im_online: Some(Default::default()),
        pallet_authority_discovery: Some(kumandra_runtime::AuthorityDiscoveryConfig {
            keys: vec![],
        }),
        pallet_sudo: Some(kumandra_runtime::SudoConfig {
            key: endowed_accounts[0].clone(),
        }),
        pallet_contracts: Some(kumandra_runtime::ContractsConfig {
            current_schedule: pallet_contracts::Schedule {
                ..Default::default()
            },
        }),
        parachains_configuration: Some(kumandra_runtime::ParachainsConfigurationConfig {
            config: indracore_runtime_parachains::configuration::HostConfiguration {
                validation_upgrade_frequency: 600u32,
                validation_upgrade_delay: 300,
                acceptance_period: 1200,
                max_code_size: 5 * 1024 * 1024,
                max_pov_size: 50 * 1024 * 1024,
                max_head_data_size: 32 * 1024,
                group_rotation_frequency: 20,
                chain_availability_period: 4,
                thread_availability_period: 4,
                no_show_slots: 10,
                max_upward_queue_count: 8,
                max_upward_queue_size: 8 * 1024,
                max_downward_message_size: 1024,
                // this is approximatelly 4ms.
                //
                // Same as `4 * frame_support::weights::WEIGHT_PER_MILLIS`. We don't bother with
                // an import since that's a made up number and should be replaced with a constant
                // obtained by benchmarking anyway.
                preferred_dispatchable_upward_messages_step_weight: 4 * 1_000_000_000,
                max_upward_message_size: 1024,
                max_upward_message_num_per_candidate: 5,
                hrmp_open_request_ttl: 5,
                hrmp_sender_deposit: 0,
                hrmp_recipient_deposit: 0,
                hrmp_channel_max_capacity: 8,
                hrmp_channel_max_total_size: 8 * 1024,
                hrmp_max_parachain_inbound_channels: 4,
                hrmp_max_parathread_inbound_channels: 4,
                hrmp_channel_max_message_size: 1024,
                hrmp_max_parachain_outbound_channels: 4,
                hrmp_max_parathread_outbound_channels: 4,
                hrmp_max_message_num_per_candidate: 5,
                ..Default::default()
            },
        }),
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

/// Kumandra staging testnet config.
pub fn kumandra_staging_testnet_config() -> Result<KumandraChainSpec, String> {
    let wasm_binary = kumandra::WASM_BINARY.ok_or("Kumandra development wasm not available")?;
    let boot_nodes = vec![];

    Ok(KumandraChainSpec::from_genesis(
        "Kumandra Staging Testnet",
        "kumandra_staging_testnet",
        ChainType::Live,
        move || KumandraGenesisExt {
            runtime_genesis_config: kumandra_staging_testnet_config_genesis(wasm_binary),
            session_length_in_blocks: None,
        },
        boot_nodes,
        Some(
            TelemetryEndpoints::new(vec![(KUMANDRA_STAGING_TELEMETRY_URL.to_string(), 0)])
                .expect("Kumandra Staging telemetry url is valid; qed"),
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
        frame_system: Some(indracore::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_indices: Some(indracore::IndicesConfig { indices: vec![] }),
        pallet_balances: Some(indracore::BalancesConfig {
            balances: endowed_accounts
                .iter()
                .map(|k| (k.clone(), ENDOWMENT))
                .collect(),
        }),
        pallet_session: Some(indracore::SessionConfig {
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
        }),
        pallet_staking: Some(indracore::StakingConfig {
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
        }),
        pallet_elections_phragmen: Some(Default::default()),
        pallet_democracy: Some(indracore::DemocracyConfig::default()),
        pallet_collective_Instance1: Some(indracore::CouncilConfig {
            members: vec![],
            phantom: Default::default(),
        }),
        pallet_collective_Instance2: Some(indracore::TechnicalCommitteeConfig {
            members: vec![],
            phantom: Default::default(),
        }),
        pallet_membership_Instance1: Some(Default::default()),
        pallet_babe: Some(Default::default()),
        pallet_grandpa: Some(Default::default()),
        pallet_im_online: Some(Default::default()),
        pallet_authority_discovery: Some(indracore::AuthorityDiscoveryConfig { keys: vec![] }),
        pallet_vesting: Some(indracore::VestingConfig { vesting: vec![] }),
        pallet_treasury: Some(Default::default()),
        pallet_sudo: Some(indracore::SudoConfig {
            key: endowed_accounts[0].clone(),
        }),
        pallet_contracts: Some(indracore::ContractsConfig {
            current_schedule: pallet_contracts::Schedule {
                ..Default::default()
            },
        }),
    }
}

/// Helper function to create kumandra GenesisConfig for testing
pub fn kumandra_testnet_genesis(
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
    root_key: AccountId,
    endowed_accounts: Option<Vec<AccountId>>,
) -> kumandra_runtime::GenesisConfig {
    let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

    const ENDOWMENT: u128 = 1_000_000 * SELS;

    kumandra_runtime::GenesisConfig {
        frame_system: Some(kumandra_runtime::SystemConfig {
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_indices: Some(kumandra_runtime::IndicesConfig { indices: vec![] }),
        pallet_balances: Some(kumandra_runtime::BalancesConfig {
            balances: endowed_accounts
                .iter()
                .map(|k| (k.clone(), ENDOWMENT))
                .collect(),
        }),
        pallet_session: Some(kumandra_runtime::SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        kumandra_session_keys(
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
        }),
        pallet_babe: Some(Default::default()),
        pallet_grandpa: Some(Default::default()),
        pallet_im_online: Some(Default::default()),
        pallet_authority_discovery: Some(kumandra_runtime::AuthorityDiscoveryConfig {
            keys: vec![],
        }),
        pallet_contracts: Some(kumandra_runtime::ContractsConfig {
            current_schedule: pallet_contracts::Schedule {
                ..Default::default()
            },
        }),
        pallet_sudo: Some(kumandra_runtime::SudoConfig { key: root_key }),
        parachains_configuration: Some(kumandra_runtime::ParachainsConfigurationConfig {
            config: indracore_runtime_parachains::configuration::HostConfiguration {
                validation_upgrade_frequency: 600u32,
                validation_upgrade_delay: 300,
                acceptance_period: 1200,
                max_code_size: 5 * 1024 * 1024,
                max_pov_size: 50 * 1024 * 1024,
                max_head_data_size: 32 * 1024,
                group_rotation_frequency: 20,
                chain_availability_period: 4,
                thread_availability_period: 4,
                no_show_slots: 10,
                max_upward_queue_count: 8,
                max_upward_queue_size: 8 * 1024,
                max_downward_message_size: 1024,
                // this is approximatelly 4ms.
                //
                // Same as `4 * frame_support::weights::WEIGHT_PER_MILLIS`. We don't bother with
                // an import since that's a made up number and should be replaced with a constant
                // obtained by benchmarking anyway.
                preferred_dispatchable_upward_messages_step_weight: 4 * 1_000_000_000,
                max_upward_message_size: 1024,
                max_upward_message_num_per_candidate: 5,
                hrmp_open_request_ttl: 5,
                hrmp_sender_deposit: 0,
                hrmp_recipient_deposit: 0,
                hrmp_channel_max_capacity: 8,
                hrmp_channel_max_total_size: 8 * 1024,
                hrmp_max_parachain_inbound_channels: 4,
                hrmp_max_parathread_inbound_channels: 4,
                hrmp_channel_max_message_size: 1024,
                hrmp_max_parachain_outbound_channels: 4,
                hrmp_max_parathread_outbound_channels: 4,
                hrmp_max_message_num_per_candidate: 5,
                ..Default::default()
            },
        }),
    }
}

fn indracore_development_config_genesis(wasm_binary: &[u8]) -> indracore::GenesisConfig {
    indracore_testnet_genesis(
        wasm_binary,
        vec![get_authority_keys_from_seed("Alice")],
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
        Some(
            serde_json::from_str(
                "{
            \"tokenDecimals\": 8,
            \"tokenSymbol\": \"SEL\"
        	}",
            )
            .expect("Provided valid json map"),
        ),
        Default::default(),
    ))
}

fn indracore_local_testnet_genesis(wasm_binary: &[u8]) -> indracore::GenesisConfig {
    indracore_testnet_genesis(
        wasm_binary,
        vec![
            get_authority_keys_from_seed("Alice"),
            get_authority_keys_from_seed("Bob"),
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
        Some(
            serde_json::from_str(
                "{
            \"tokenDecimals\": 8,
            \"tokenSymbol\": \"SEL\"
        	}",
            )
            .expect("Provided valid json map"),
        ),
        Default::default(),
    ))
}

fn kumandra_local_testnet_genesis(wasm_binary: &[u8]) -> kumandra_runtime::GenesisConfig {
    kumandra_testnet_genesis(
        wasm_binary,
        vec![
            get_authority_keys_from_seed("Alice"),
            get_authority_keys_from_seed("Bob"),
        ],
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        None,
    )
}

/// Kumandra local testnet config (multivalidator Alice + Bob)
pub fn kumandra_local_testnet_config() -> Result<KumandraChainSpec, String> {
    let wasm_binary = kumandra::WASM_BINARY.ok_or("Kumandra development wasm not available")?;

    Ok(KumandraChainSpec::from_genesis(
        "Kumandra Local Testnet",
        "kumandra_local_testnet",
        ChainType::Local,
        move || KumandraGenesisExt {
            runtime_genesis_config: kumandra_local_testnet_genesis(wasm_binary),
            // Use 1 minute session length.
            session_length_in_blocks: Some(10),
        },
        vec![],
        None,
        Some(DEFAULT_PROTOCOL_ID),
        None,
        Default::default(),
    ))
}
