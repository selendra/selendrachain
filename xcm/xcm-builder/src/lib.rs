// Copyright 2020 Parity Technologies (UK) Ltd.
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

#![cfg_attr(not(feature = "std"), no_std)]

mod location_conversion;
pub use location_conversion::{
    Account32Hash, AccountId32Aliases, ChildParachainConvertsVia, ParentIsDefault,
    SiblingParachainConvertsVia,
};

mod origin_conversion;
pub use origin_conversion::{
    ChildParachainAsNative, ChildSystemParachainAsSuperuser, ParentAsSuperuser, RelayChainAsNative,
    SiblingParachainAsNative, SiblingSystemParachainAsSuperuser, SignedAccountId32AsNative,
    SovereignSignedViaLocation,
};

mod currency_adapter;
pub use currency_adapter::CurrencyAdapter;

use frame_support::traits::Get;
use sp_std::marker::PhantomData;
use xcm::v0::{Junction, MultiLocation};
use xcm_executor::traits::InvertLocation;

/// Simple location inverter; give it this location's ancestry and it'll
pub struct LocationInverter<Ancestry>(PhantomData<Ancestry>);

impl<Ancestry: Get<MultiLocation>> InvertLocation for LocationInverter<Ancestry> {
    fn invert_location(location: &MultiLocation) -> MultiLocation {
        let mut ancestry = Ancestry::get();
        let mut result = location.clone();
        for (i, j) in location
            .iter_rev()
            .map(|j| match j {
                Junction::Parent => ancestry.take_first().unwrap_or(Junction::OnlyChild),
                _ => Junction::Parent,
            })
            .enumerate()
        {
            *result
                .at_mut(i)
                .expect("location and result begin equal; same size; qed") = j;
        }
        result
    }
}
