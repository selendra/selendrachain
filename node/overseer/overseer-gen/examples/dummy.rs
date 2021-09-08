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

//! A dummy to be used with cargo expand

use selendra_node_network_protocol::WrongVariant;
use selendra_overseer_gen::*;

/// Concrete subsystem implementation for `MsgStrukt` msg type.
#[derive(Default)]
pub struct AwesomeSubSys;

impl ::selendra_overseer_gen::Subsystem<XxxSubsystemContext<MsgStrukt>, Yikes> for AwesomeSubSys {
	fn start(self, _ctx: XxxSubsystemContext<MsgStrukt>) -> SpawnedSubsystem<Yikes> {
		unimplemented!("starting yay!")
	}
}

#[derive(Default)]
pub struct GoblinTower;

impl ::selendra_overseer_gen::Subsystem<XxxSubsystemContext<Plinko>, Yikes> for GoblinTower {
	fn start(self, _ctx: XxxSubsystemContext<Plinko>) -> SpawnedSubsystem<Yikes> {
		unimplemented!("welcum")
	}
}

/// A signal sent by the overseer.
#[derive(Debug, Clone)]
pub struct SigSigSig;

/// The external event.
#[derive(Debug, Clone)]
pub struct EvX;

impl EvX {
	pub fn focus<'a, T>(&'a self) -> Result<EvX, ()> {
		unimplemented!("dispatch")
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Yikes;

impl std::fmt::Display for Yikes {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "yikes!")
	}
}

impl std::error::Error for Yikes {}

impl From<selendra_overseer_gen::OverseerError> for Yikes {
	fn from(_: selendra_overseer_gen::OverseerError) -> Yikes {
		Yikes
	}
}

impl From<selendra_overseer_gen::mpsc::SendError> for Yikes {
	fn from(_: selendra_overseer_gen::mpsc::SendError) -> Yikes {
		Yikes
	}
}

#[derive(Debug, Clone)]
pub struct MsgStrukt(u8);

#[derive(Debug, Clone, Copy)]
pub struct Plinko;

impl From<NetworkMsg> for MsgStrukt {
	fn from(_event: NetworkMsg) -> Self {
		MsgStrukt(1u8)
	}
}

#[derive(Debug, Clone, Copy)]
pub enum NetworkMsg {
	A,
	B,
	C,
}

impl NetworkMsg {
	fn focus(&self) -> Result<Self, WrongVariant> {
		Ok(match self {
			Self::B => return Err(WrongVariant),
			Self::A | Self::C => self.clone(),
		})
	}
}

#[overlord(signal=SigSigSig, event=EvX, error=Yikes, network=NetworkMsg, gen=AllMessages)]
struct Xxx {
	#[subsystem(MsgStrukt)]
	sub0: AwesomeSubSys,

	#[subsystem(no_dispatch, blocking, Plinko)]
	plinkos: GoblinTower,

	i_like_pi: f64,
}

#[derive(Debug, Clone)]
struct DummySpawner;

impl SpawnNamed for DummySpawner {
	fn spawn_blocking(&self, name: &'static str, _future: futures::future::BoxFuture<'static, ()>) {
		unimplemented!("spawn blocking {}", name)
	}

	fn spawn(&self, name: &'static str, _future: futures::future::BoxFuture<'static, ()>) {
		unimplemented!("spawn {}", name)
	}
}

#[derive(Debug, Clone)]
struct DummyCtx;

fn main() {
	let (overseer, _handle): (Xxx<_>, _) = Xxx::builder()
		.sub0(AwesomeSubSys::default())
		.plinkos(GoblinTower::default())
		.i_like_pi(::std::f64::consts::PI)
		.spawner(DummySpawner)
		.build()
		.unwrap();
	assert_eq!(overseer.i_like_pi.floor() as i8, 3);
}
