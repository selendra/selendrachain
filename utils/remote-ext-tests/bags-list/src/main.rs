// Copyright 2021 SmallWorld Selendra (Kh).
// This file is part of Selendra.

// Selendra is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Selendra is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Selendra.  If not, see <http://www.gnu.org/licenses/>.

//! Remote tests for bags-list pallet.

use clap::{ArgEnum, Parser};
use std::convert::TryInto;

#[derive(Clone, Debug, ArgEnum)]
#[clap(rename_all = "PascalCase")]
enum Command {
	CheckMigration,
	SanityCheck,
	Snapshot,
}

#[derive(Clone, Debug, ArgEnum)]
#[clap(rename_all = "PascalCase")]
enum Runtime {
	Selendra,
	Cardamom,
}

#[derive(Parser)]
struct Cli {
	#[clap(long, short, default_value = "wss://cardamom-rpc.selendra.io:443")]
	uri: String,
	#[clap(long, short, ignore_case = true, arg_enum, default_value = "cardamom")]
	runtime: Runtime,
	#[clap(long, short, ignore_case = true, arg_enum, default_value = "SanityCheck")]
	command: Command,
	#[clap(long, short)]
	snapshot_limit: Option<usize>,
}

#[tokio::main]
async fn main() {
	let options = Cli::parse();
	sp_tracing::try_init_simple();

	log::info!(
		target: "remote-ext-tests",
		"using runtime {:?} / command: {:?}",
		options.runtime,
		options.command
	);

	use pallet_bags_list_remote_tests::*;
	match options.runtime {
		Runtime::Selendra => sp_core::crypto::set_default_ss58_version(
			<selendra_runtime::Runtime as frame_system::Config>::SS58Prefix::get()
				.try_into()
				.unwrap(),
		),
		Runtime::Cardamom => sp_core::crypto::set_default_ss58_version(
			<cardamom_runtime::Runtime as frame_system::Config>::SS58Prefix::get()
				.try_into()
				.unwrap(),
		),
	};

	match (options.runtime, options.command) {
		(Runtime::Cardamom, Command::CheckMigration) => {
			use cardamom_runtime::{Block, Runtime};
			use cardamom_runtime_constants::currency::UNITS;
			migration::execute::<Runtime, Block>(UNITS as u64, "CDM", options.uri.clone()).await;
		},
		(Runtime::Cardamom, Command::SanityCheck) => {
			use cardamom_runtime::{Block, Runtime};
			use cardamom_runtime_constants::currency::UNITS;
			sanity_check::execute::<Runtime, Block>(UNITS as u64, "CDM", options.uri.clone()).await;
		},
		(Runtime::Cardamom, Command::Snapshot) => {
			use cardamom_runtime::{Block, Runtime};
			use cardamom_runtime_constants::currency::UNITS;
			snapshot::execute::<Runtime, Block>(
				options.snapshot_limit,
				UNITS.try_into().unwrap(),
				options.uri.clone(),
			)
			.await;
		},

		(Runtime::Selendra, Command::CheckMigration) => {
			use selendra_runtime::{Block, Runtime};
			use selendra_runtime_constants::currency::UNITS;
			migration::execute::<Runtime, Block>(UNITS as u64, "SEL", options.uri.clone()).await;
		},
		(Runtime::Selendra, Command::SanityCheck) => {
			use selendra_runtime::{Block, Runtime};
			use selendra_runtime_constants::currency::UNITS;
			sanity_check::execute::<Runtime, Block>(UNITS as u64, "SEL", options.uri.clone()).await;
		},
		(Runtime::Selendra, Command::Snapshot) => {
			use selendra_runtime::{Block, Runtime};
			use selendra_runtime_constants::currency::UNITS;
			snapshot::execute::<Runtime, Block>(
				options.snapshot_limit,
				UNITS.try_into().unwrap(),
				options.uri.clone(),
			)
			.await;
		},
	}
}
