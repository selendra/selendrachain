
//! Autogenerated weights for `runtime_parachains::disputes`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-05-26, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("selendra-dev"), DB CACHE: 1024

// Executed Command:
// target/release/selendra
// benchmark
// --chain=selendra-dev
// --steps=50
// --repeat=20
// --pallet=runtime_parachains::disputes
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=./runtime/selendra/src/weights/runtime_parachains_disputes.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `runtime_parachains::disputes`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> runtime_parachains::disputes::WeightInfo for WeightInfo<T> {
	// Storage: ParasDisputes Frozen (r:0 w:1)
	fn force_unfreeze() -> Weight {
		(2_068_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}
