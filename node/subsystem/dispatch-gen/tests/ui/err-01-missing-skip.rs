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
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

#![allow(dead_code)]

use polkadot_procmacro_subsystem_dispatch_gen::subsystem_dispatch_gen;

/// The event type in question.
#[derive(Clone, Copy)]
enum Event {
	Smth,
	Else,
}

impl Event {
	fn focus(&self) -> std::result::Result<Inner, ()> {
		unimplemented!("foo")
	}
}

/// This should have a `From<Event>` impl but does not.
#[derive(Clone)]
enum Inner {
	Foo,
	Bar(Event),
}

#[subsystem_dispatch_gen(Event)]
#[derive(Clone)]
enum AllMessages {
	/// Foo
	Vvvvvv(Inner),

    /// Missing a `#[skip]` annotation
    Uuuuu,
}

fn main() {
    let _x = AllMessages::dispatch_iter(Event::Else);
}
