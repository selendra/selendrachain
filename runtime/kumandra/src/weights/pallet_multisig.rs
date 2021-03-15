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
//! Autogenerated weights for pallet_multisig
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-02-22, STEPS: [50, ], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("kumandra-dev"), DB CACHE: 128

// Executed Command:
// target/release/indracore
// benchmark
// --chain=kumandra-dev
// --steps=50
// --repeat=20
// --pallet=pallet_multisig
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --header=./file_header.txt
// --output=./runtime/westend/src/weights/

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for pallet_multisig.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_multisig::WeightInfo for WeightInfo<T> {
    fn as_multi_threshold_1(z: u32) -> Weight {
        (13_586_000 as Weight)
            // Standard Error: 0
            .saturating_add((1_000 as Weight).saturating_mul(z as Weight))
    }
    fn as_multi_create(s: u32, z: u32) -> Weight {
        (53_651_000 as Weight)
            // Standard Error: 0
            .saturating_add((119_000 as Weight).saturating_mul(s as Weight))
            // Standard Error: 0
            .saturating_add((1_000 as Weight).saturating_mul(z as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn as_multi_create_store(s: u32, z: u32) -> Weight {
        (60_007_000 as Weight)
            // Standard Error: 0
            .saturating_add((123_000 as Weight).saturating_mul(s as Weight))
            // Standard Error: 0
            .saturating_add((3_000 as Weight).saturating_mul(z as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn as_multi_approve(s: u32, z: u32) -> Weight {
        (31_325_000 as Weight)
            // Standard Error: 0
            .saturating_add((139_000 as Weight).saturating_mul(s as Weight))
            // Standard Error: 0
            .saturating_add((1_000 as Weight).saturating_mul(z as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn as_multi_approve_store(s: u32, z: u32) -> Weight {
        (57_283_000 as Weight)
            // Standard Error: 0
            .saturating_add((154_000 as Weight).saturating_mul(s as Weight))
            // Standard Error: 0
            .saturating_add((3_000 as Weight).saturating_mul(z as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn as_multi_complete(s: u32, z: u32) -> Weight {
        (74_192_000 as Weight)
            // Standard Error: 0
            .saturating_add((267_000 as Weight).saturating_mul(s as Weight))
            // Standard Error: 0
            .saturating_add((5_000 as Weight).saturating_mul(z as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn approve_as_multi_create(s: u32) -> Weight {
        (53_042_000 as Weight)
            // Standard Error: 0
            .saturating_add((123_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn approve_as_multi_approve(s: u32) -> Weight {
        (30_731_000 as Weight)
            // Standard Error: 0
            .saturating_add((139_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn approve_as_multi_complete(s: u32) -> Weight {
        (147_692_000 as Weight)
            // Standard Error: 0
            .saturating_add((277_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn cancel_as_multi(s: u32) -> Weight {
        (100_323_000 as Weight)
            // Standard Error: 0
            .saturating_add((129_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
}
