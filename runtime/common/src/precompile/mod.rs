//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

#![allow(clippy::upper_case_acronyms)]

use module_evm::{
	precompiles::{
		Precompile, Precompiles, EvmPrecompiles,
		Identity,
		Ripemd160, Sha256,
		Sha3FIPS256, Sha3FIPS512,
		ECRecover, ECRecoverPublicKey,
	},
	Context, ExitError, ExitSucceed,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
use primitives::v1::{
	PRECOMPILE_ADDRESS_START, PREDEPLOY_ADDRESS_START
};
use sp_core::H160;
use sp_std::{marker::PhantomData, prelude::*};
use sp_runtime::traits::Convert;
use frame_support::weights::Weight;

pub mod input;
pub mod multicurrency;
pub mod schedule_call;
pub mod state_rent;

pub use multicurrency::MultiCurrencyPrecompile;
pub use schedule_call::ScheduleCallPrecompile;
pub use state_rent::StateRentPrecompile;
pub use module_support::{PrecompileCallerFilter};

pub const SYSTEM_CONTRACT_LEADING_ZERO_BYTES: usize = 12;

// Check if the given `address` is a system contract.
///
/// It's system contract if the address starts with 12 zero bytes.
pub fn is_system_contract(address: H160) -> bool {
	address[..SYSTEM_CONTRACT_LEADING_ZERO_BYTES] == [0u8; SYSTEM_CONTRACT_LEADING_ZERO_BYTES]
}

pub fn is_core_precompile(address: H160) -> bool {
	address >= H160::from_low_u64_be(PRECOMPILE_ADDRESS_START)
		&& address < H160::from_low_u64_be(PREDEPLOY_ADDRESS_START)
}

/// The call is allowed only if caller is a system contract.
pub struct SystemContractsFilter;
impl PrecompileCallerFilter for SystemContractsFilter {
	fn is_allowed(caller: H160) -> bool {
		is_system_contract(caller)
	}
}

/// Convert gas to weight
pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> u64 {
		a as Weight
	}
}

pub struct AllPrecompiles<
	PrecompileCallerFilter,
	MultiCurrencyPrecompile,
	StateRentPrecompile,
	ScheduleCallPrecompile,
>(
	PhantomData<(
		PrecompileCallerFilter,
		MultiCurrencyPrecompile,
		StateRentPrecompile,
		ScheduleCallPrecompile,
	)>,
);

impl<
		PrecompileCallerFilter,
		MultiCurrencyPrecompile,
		StateRentPrecompile,
		ScheduleCallPrecompile,
	> Precompiles
	for AllPrecompiles<
		PrecompileCallerFilter,
		MultiCurrencyPrecompile,
		StateRentPrecompile,
		ScheduleCallPrecompile,
	> where
	MultiCurrencyPrecompile: Precompile,
	StateRentPrecompile: Precompile,
	ScheduleCallPrecompile: Precompile,
	PrecompileCallerFilter: PrecompileCallerFilterT,
{
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
		EvmPrecompiles::<ECRecover, Sha256, Ripemd160, Identity, ECRecoverPublicKey, Sha3FIPS256, Sha3FIPS512>::execute(
			address, input, target_gas, context,
		)
		.or_else(|| {
			if is_core_precompile(address) && !PrecompileCallerFilter::is_allowed(context.caller) {
				log::debug!(target: "evm", "Precompile no permission");
				return Some(Err(ExitError::Other("no permission".into())));
			}

			if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START) {
				Some(MultiCurrencyPrecompile::execute(input, target_gas, context))
			} else if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START + 2) {
				Some(StateRentPrecompile::execute(input, target_gas, context))
			} else if address == H160::from_low_u64_be(PRECOMPILE_ADDRESS_START + 4) {
				Some(ScheduleCallPrecompile::execute(input, target_gas, context))
			} else {
				None
			}
		})
	}
}
