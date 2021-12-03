// Copyright 2021 Parity Technologies (UK) Ltd.
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
// along with Polkadot. If not, see <http://www.gnu.org/licenses/>.

//! Tests for the Selendra Runtime Configuration

use crate::*;
use frame_support::weights::{GetDispatchInfo, WeightToFeePolynomial};
use pallet_transaction_payment::Multiplier;
use parity_scale_codec::Encode;
use separator::Separatable;
use sp_runtime::FixedPointNumber;

#[test]
fn payout_weight_portion() {
	use pallet_staking::WeightInfo;
	let payout_weight = <Runtime as pallet_staking::Config>::WeightInfo::payout_stakers_alive_staked(
		MaxNominatorRewardedPerValidator::get(),
	) as f64;
	let block_weight = BlockWeights::get().max_block as f64;

	println!(
		"a full payout takes {:.2} of the block weight [{} / {}]",
		payout_weight / block_weight,
		payout_weight,
		block_weight
	);
	assert!(payout_weight * 2f64 < block_weight);
}

#[test]
#[ignore]
fn block_cost() {
	let max_block_weight = BlockWeights::get().max_block;
	let raw_fee = WeightToFee::calc(&max_block_weight);

	println!(
		"Full Block weight == {} // WeightToFee(full_block) == {} plank",
		max_block_weight,
		raw_fee.separated_string(),
	);
}

#[test]
#[ignore]
fn transfer_cost_min_multiplier() {
	let min_multiplier = runtime_common::MinimumMultiplier::get();
	let call = pallet_balances::Call::<Runtime>::transfer_keep_alive {
		dest: Default::default(),
		value: Default::default(),
	};
	let info = call.get_dispatch_info();
	// convert to outer call.
	let call = Call::Balances(call);
	let len = call.using_encoded(|e| e.len()) as u32;

	let mut ext = sp_io::TestExternalities::new_empty();
	let mut test_with_multiplier = |m| {
		ext.execute_with(|| {
			pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(m);
			let fee = TransactionPayment::compute_fee(len, &info, 0);
			println!(
				"weight = {:?} // multiplier = {:?} // full transfer fee = {:?}",
				info.weight.separated_string(),
				pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get(),
				fee.separated_string(),
			);
		});
	};

	test_with_multiplier(min_multiplier);
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000_000u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000_000_000u128));
}

#[test]
fn nominator_limit() {
	use pallet_election_provider_multi_phase::WeightInfo;
	// starting point of the nominators.
	let all_voters: u32 = 10_000;

	// assuming we want around 5k candidates and 1k active validators.
	let all_targets: u32 = 5_000;
	let desired: u32 = 1_000;
	let weight_with = |active| {
		<Runtime as pallet_election_provider_multi_phase::Config>::WeightInfo::submit_unsigned(
			all_voters.max(active),
			all_targets,
			active,
			desired,
		)
	};

	let mut active = 1;
	while weight_with(active) <= OffchainSolutionWeightLimit::get() || active == all_voters {
		active += 1;
	}

	println!("can support {} nominators to yield a weight of {}", active, weight_with(active));
}

#[test]
fn call_size() {
	assert!(
		core::mem::size_of::<Call>() <= 230,
		"size of Call is more than 230 bytes: some calls have too big arguments, use Box to reduce \
		the size of Call.
		If the limit is too strong, maybe consider increase the limit to 300.",
	);
}
