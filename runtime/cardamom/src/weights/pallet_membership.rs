// Copyright 2017-2022 Parity Technologies (UK) Ltd.
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
//! Autogenerated weights for `pallet_membership`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-03-15, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("cardamom-dev"), DB CACHE: 1024

// Executed Command:
// ./target/production/selendra
// benchmark
// --chain=cardamom-dev
// --steps=50
// --repeat=20
// --pallet=pallet_membership
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --header=./file_header.txt
// --output=./runtime/cardamom/src/weights/pallet_membership.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_membership`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_membership::WeightInfo for WeightInfo<T> {
	// Storage: TechnicalMembership Members (r:1 w:1)
	// Storage: TechnicalCommittee Proposals (r:1 w:0)
	// Storage: TechnicalCommittee Members (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn add_member(m: u32, ) -> Weight {
		(13_328_000 as Weight)
			// Standard Error: 0
			.saturating_add((59_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: TechnicalMembership Members (r:1 w:1)
	// Storage: TechnicalCommittee Proposals (r:1 w:0)
	// Storage: TechnicalMembership Prime (r:1 w:0)
	// Storage: TechnicalCommittee Members (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn remove_member(m: u32, ) -> Weight {
		(15_684_000 as Weight)
			// Standard Error: 0
			.saturating_add((45_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: TechnicalMembership Members (r:1 w:1)
	// Storage: TechnicalCommittee Proposals (r:1 w:0)
	// Storage: TechnicalMembership Prime (r:1 w:0)
	// Storage: TechnicalCommittee Members (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn swap_member(m: u32, ) -> Weight {
		(15_616_000 as Weight)
			// Standard Error: 0
			.saturating_add((57_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: TechnicalMembership Members (r:1 w:1)
	// Storage: TechnicalCommittee Proposals (r:1 w:0)
	// Storage: TechnicalMembership Prime (r:1 w:0)
	// Storage: TechnicalCommittee Members (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn reset_member(m: u32, ) -> Weight {
		(15_248_000 as Weight)
			// Standard Error: 2_000
			.saturating_add((195_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: TechnicalMembership Members (r:1 w:1)
	// Storage: TechnicalCommittee Proposals (r:1 w:0)
	// Storage: TechnicalMembership Prime (r:1 w:1)
	// Storage: TechnicalCommittee Members (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn change_key(m: u32, ) -> Weight {
		(16_174_000 as Weight)
			// Standard Error: 0
			.saturating_add((52_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	// Storage: TechnicalMembership Members (r:1 w:0)
	// Storage: TechnicalMembership Prime (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn set_prime(m: u32, ) -> Weight {
		(4_206_000 as Weight)
			// Standard Error: 0
			.saturating_add((29_000 as Weight).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: TechnicalMembership Prime (r:0 w:1)
	// Storage: TechnicalCommittee Prime (r:0 w:1)
	fn clear_prime(_m: u32, ) -> Weight {
		(1_215_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
}
