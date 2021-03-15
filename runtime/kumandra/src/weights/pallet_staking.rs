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
//! Autogenerated weights for pallet_staking
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-02-23, STEPS: [50, ], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("kumandra-dev"), DB CACHE: 128

// Executed Command:
// target/release/indracore
// benchmark
// --chain=kumandra-dev
// --steps=50
// --repeat=20
// --pallet=pallet_staking
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

/// Weight functions for pallet_staking.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_staking::WeightInfo for WeightInfo<T> {
    fn bond() -> Weight {
        (76_655_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn bond_extra() -> Weight {
        (62_697_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn unbond() -> Weight {
        (57_677_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_update(s: u32) -> Weight {
        (58_550_000 as Weight)
            // Standard Error: 0
            .saturating_add((33_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn withdraw_unbonded_kill(s: u32) -> Weight {
        (90_608_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((2_620_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(7 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn validate() -> Weight {
        (19_249_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn kick(k: u32) -> Weight {
        (16_162_000 as Weight)
            // Standard Error: 7_000
            .saturating_add((18_595_000 as Weight).saturating_mul(k as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(k as Weight)))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(k as Weight)))
    }
    fn nominate(n: u32) -> Weight {
        (29_674_000 as Weight)
            // Standard Error: 12_000
            .saturating_add((5_946_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn chill() -> Weight {
        (18_554_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn set_payee() -> Weight {
        (12_732_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_controller() -> Weight {
        (28_004_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn set_validator_count() -> Weight {
        (2_289_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_no_eras() -> Weight {
        (2_485_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era() -> Weight {
        (2_531_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_new_era_always() -> Weight {
        (2_570_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_invulnerables(v: u32) -> Weight {
        (2_608_000 as Weight)
            // Standard Error: 0
            .saturating_add((35_000 as Weight).saturating_mul(v as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn force_unstake(s: u32) -> Weight {
        (61_375_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((2_596_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn cancel_deferred_slash(s: u32) -> Weight {
        (5_908_427_000 as Weight)
            // Standard Error: 389_000
            .saturating_add((34_633_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn payout_stakers_dead_controller(n: u32) -> Weight {
        (113_072_000 as Weight)
            // Standard Error: 18_000
            .saturating_add((49_690_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(11 as Weight))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(n as Weight)))
    }
    fn payout_stakers_alive_staked(n: u32) -> Weight {
        (139_374_000 as Weight)
            // Standard Error: 20_000
            .saturating_add((64_080_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(12 as Weight))
            .saturating_add(T::DbWeight::get().reads((5 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
            .saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(n as Weight)))
    }
    fn rebond(l: u32) -> Weight {
        (40_440_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((82_000 as Weight).saturating_mul(l as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn set_history_depth(e: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 62_000
            .saturating_add((32_193_000 as Weight).saturating_mul(e as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
            .saturating_add(T::DbWeight::get().writes((7 as Weight).saturating_mul(e as Weight)))
    }
    fn reap_stash(s: u32) -> Weight {
        (65_018_000 as Weight)
            // Standard Error: 1_000
            .saturating_add((2_582_000 as Weight).saturating_mul(s as Weight))
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as Weight)))
    }
    fn new_era(v: u32, n: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 817_000
            .saturating_add((579_390_000 as Weight).saturating_mul(v as Weight))
            // Standard Error: 41_000
            .saturating_add((82_050_000 as Weight).saturating_mul(n as Weight))
            .saturating_add(T::DbWeight::get().reads(7 as Weight))
            .saturating_add(T::DbWeight::get().reads((4 as Weight).saturating_mul(v as Weight)))
            .saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes(8 as Weight))
            .saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(v as Weight)))
    }
    fn submit_solution_better(v: u32, n: u32, a: u32, w: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 51_000
            .saturating_add((635_000 as Weight).saturating_mul(v as Weight))
            // Standard Error: 20_000
            .saturating_add((334_000 as Weight).saturating_mul(n as Weight))
            // Standard Error: 51_000
            .saturating_add((75_418_000 as Weight).saturating_mul(a as Weight))
            // Standard Error: 106_000
            .saturating_add((6_813_000 as Weight).saturating_mul(w as Weight))
            .saturating_add(T::DbWeight::get().reads(6 as Weight))
            .saturating_add(T::DbWeight::get().reads((4 as Weight).saturating_mul(a as Weight)))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(w as Weight)))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
}
