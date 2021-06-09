
//! Autogenerated weights for pallet_utility
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-05-31, STEPS: `[50, ]`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("statemine-dev"), DB CACHE: 128

// Executed Command:
// ./target/release/selendra-parachain
// benchmark
// --chain=statemine-dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet=pallet_utility
// --extrinsic=*
// --steps=50
// --repeat=20
// --raw
// --output=./selendra-parachain/src/weights/


#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for pallet_utility.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_utility::WeightInfo for WeightInfo<T> {
	fn batch(c: u32, ) -> Weight {
		(13_271_000 as Weight)
			// Standard Error: 0
			.saturating_add((1_036_000 as Weight).saturating_mul(c as Weight))
	}
	fn as_derivative() -> Weight {
		(3_577_000 as Weight)
	}
	fn batch_all(c: u32, ) -> Weight {
		(14_195_000 as Weight)
			// Standard Error: 0
			.saturating_add((1_044_000 as Weight).saturating_mul(c as Weight))
	}
}
