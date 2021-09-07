// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

/// Money matters.
pub mod currency {
	use primitives::v0::Balance;

	pub const UNITS: Balance = 1_000_000_000_000_000_000;
	pub const CENTS: Balance = UNITS / 10_000;
	pub const MILLICENTS: Balance = CENTS / 1_000;
	pub const NANO: Balance = MILLICENTS / 1000;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 5_000 * CENTS + (bytes as Balance) * 50 * MILLICENTS
	}
}

/// Time and blocks.
pub mod time {
	use primitives::v0::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 6000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = 4 * HOURS;

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::v0::Balance;
	use runtime_common::ExtrinsicBaseWeight;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, MAXIMUM_BLOCK_WEIGHT]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = 100 * super::currency::MILLICENTS;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}

pub mod permission {
	use crate::CouncilCollective;
	use frame_system::{EnsureOneOf, EnsureRoot};
	use primitives::v0::AccountId;
	use sp_core::u32_trait::{_1, _2, _3, _5};

	pub type ApproveOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<_3, _5, AccountId, CouncilCollective>,
	>;

	pub type MoreThanHalfCouncil = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
	>;

	pub type ScheduleOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>,
	>;

	pub type SlashCancelOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>,
	>;

	pub type AuctionInitiate = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>,
	>;
}

pub mod merge_account {
	use crate::Balances;
	use evm_accounts::account::MergeAccount;
	use frame_support::{traits::ReservableCurrency, transactional};
	use primitives::v1::AccountId;
	use sp_runtime::DispatchResult;

	pub struct MergeAccountEvm;
	impl MergeAccount<AccountId> for MergeAccountEvm {
		#[transactional]
		fn merge_account(source: &AccountId, dest: &AccountId) -> DispatchResult {
			// unreserve all reserved currency
			<Balances as ReservableCurrency<_>>::unreserve(
				source,
				Balances::reserved_balance(source),
			);

			// transfer all free to dest
			match Balances::transfer(
				Some(source.clone()).into(),
				dest.clone().into(),
				Balances::free_balance(source),
			) {
				Ok(_) => Ok(()),
				Err(e) => Err(e.error),
			}
		}
	}
}

pub mod precompiles {
	use evm::{executor::PrecompileOutput, Context, ExitError};
	use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
	use pallet_evm::{Precompile, PrecompileSet};
	use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
	use pallet_evm_precompile_dispatch::Dispatch;
	use pallet_evm_precompile_modexp::Modexp;
	use pallet_evm_precompile_sha3fips::Sha3FIPS256;
	use pallet_evm_precompile_simple::{
		ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256,
	};
	use parity_scale_codec::Decode;
	use sp_core::H160;
	use sp_std::{fmt::Debug, marker::PhantomData};
	#[derive(Debug, Clone, Copy)]
	pub struct SelendraPrecompiles<R>(PhantomData<R>);

	impl<R: frame_system::Config> SelendraPrecompiles<R> {
		/// Return all addresses that contain precompiles. This can be used to
		/// populate dummy code under the precompile, and potentially in the future
		/// to prevent using accounts that have precompiles at their addresses
		/// explicitly using something like SignedExtra.
		#[allow(dead_code)]
		fn used_addresses() -> impl Iterator<Item = H160> {
			sp_std::vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 1024, 1025, 1026, 1027, 1028, 1029]
				.into_iter()
				.map(|x| hash(x).into())
		}
	}

	/// The following distribution has been decided for the precompiles
	/// 0-1023: Ethereum Mainnet Precompiles
	/// 1024-2047 Precompiles that are not in Ethereum Mainnet but are neither
	/// Selendra specific
	impl<R: frame_system::Config + pallet_evm::Config> PrecompileSet for SelendraPrecompiles<R>
	where
		R::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
		<R::Call as Dispatchable>::Origin: From<Option<R::AccountId>>,
	{
		fn execute(
			address: H160,
			input: &[u8],
			target_gas: Option<u64>,
			context: &Context,
		) -> Option<core::result::Result<PrecompileOutput, ExitError>> {
			match address {
				// Ethereum precompiles
				a if a == hash(1) => Some(ECRecover::execute(input, target_gas, context)),
				a if a == hash(2) => Some(Sha256::execute(input, target_gas, context)),
				a if a == hash(3) => Some(Ripemd160::execute(input, target_gas, context)),
				a if a == hash(4) => Some(Identity::execute(input, target_gas, context)),
				a if a == hash(5) => Some(Modexp::execute(input, target_gas, context)),
				a if a == hash(6) => Some(Bn128Add::execute(input, target_gas, context)),
				a if a == hash(7) => Some(Bn128Mul::execute(input, target_gas, context)),
				a if a == hash(8) => Some(Bn128Pairing::execute(input, target_gas, context)),
				// Non-Selendra specific nor Ethereum precompiles :
				a if a == hash(1024) => Some(Sha3FIPS256::execute(input, target_gas, context)),
				a if a == hash(1025) => Some(Dispatch::<R>::execute(input, target_gas, context)),
				a if a == hash(1026) =>
					Some(ECRecoverPublicKey::execute(input, target_gas, context)),
				_ => None,
			}
		}
	}

	fn hash(a: u64) -> H160 {
		H160::from_low_u64_be(a)
	}
}

#[cfg(test)]
mod tests {
	use super::{
		currency::{CENTS, MILLICENTS},
		fee::WeightToFee,
	};
	use frame_support::weights::WeightToFeePolynomial;
	use runtime_common::{ExtrinsicBaseWeight, MAXIMUM_BLOCK_WEIGHT};

	#[test]
	// This function tests that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 1,600 CENTS
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&MAXIMUM_BLOCK_WEIGHT);
		let y = 16 * 100 * CENTS;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::calc(&ExtrinsicBaseWeight::get());
		let y = CENTS / 10;
		assert!(x.max(y) - x.min(y) < MILLICENTS);
	}
}
