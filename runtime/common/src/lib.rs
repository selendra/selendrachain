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

//! Common runtime code for Indracore.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod impls;

pub use frame_support::weights::constants::{
    BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight,
};
use frame_support::{
    parameter_types,
    traits::Currency,
    weights::{constants::WEIGHT_PER_SECOND, DispatchClass, Weight},
};
use frame_system::limits;
use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
use primitives::v1::{BlockNumber, ValidatorId};
use sp_runtime::{FixedPointNumber, Perbill, Perquintill};
use static_assertions::const_assert;

pub use pallet_balances::Call as BalancesCall;
#[cfg(feature = "std")]
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

/// Implementations of some helper traits passed into runtime modules as associated types.
pub use impls::ToAuthor;

pub type NegativeImbalance<T> = <pallet_balances::Module<T> as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

// Common constants used in all runtimes.
parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    /// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
    /// than this will decrease the weight and more will increase.
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    /// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
    /// change the fees more rapidly.
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
    /// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
    /// that combined with `AdjustmentVariable`, we can recover from the minimum.
    /// See `multiplier_can_grow_from_zero`.
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
    /// Maximum length of block. Up to 5MB.
    pub BlockLength: limits::BlockLength =
        limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    /// Block weights base values and limits.
    pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have an extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
}

parameter_types! {
    /// A limit for off-chain phragmen unsigned solution submission.
    ///
    /// We want to keep it as high as possible, but can't risk having it reject,
    /// so we always subtract the base block execution weight.
    pub OffchainSolutionWeightLimit: Weight = BlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic
        .expect("Normal extrinsics have weight limit configured by default; qed")
        .saturating_sub(BlockExecutionWeight::get());
}

/// Parameterized slow adjusting fee updated based on
/// https://w3f-research.readthedocs.io/en/latest/polkadot/economics/1-token-economics.html
pub type SlowAdjustingFeeUpdate<R> =
    TargetedFeeAdjustment<R, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;

/// The type used for currency conversion.
///
/// This must only be used as long as the balance type is u128.
pub type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
static_assertions::assert_eq_size!(primitives::v1::Balance, u128);

/// A placeholder since there is currently no provided session key handler for parachain validator
/// keys.
pub struct ParachainSessionKeyPlaceholder<T>(sp_std::marker::PhantomData<T>);
impl<T> sp_runtime::BoundToRuntimeAppPublic for ParachainSessionKeyPlaceholder<T> {
    type Public = ValidatorId;
}

impl<T: pallet_session::Config> pallet_session::OneSessionHandler<T::AccountId>
    for ParachainSessionKeyPlaceholder<T>
{
    type Key = ValidatorId;

    fn on_genesis_session<'a, I: 'a>(_validators: I)
    where
        I: Iterator<Item = (&'a T::AccountId, ValidatorId)>,
        T::AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_changed: bool, _v: I, _q: I)
    where
        I: Iterator<Item = (&'a T::AccountId, ValidatorId)>,
        T::AccountId: 'a,
    {
    }

    fn on_disabled(_: usize) {}
}
