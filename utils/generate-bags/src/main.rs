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

//! Make the set of voting bag thresholds to be used in `voter_bags.rs`.
//!
//! Generally speaking this script can be run once per runtime and never
//! touched again. It can be reused to regenerate a wholly different
//! quantity of bags, or if the existential deposit changes, etc.

use cardamom_runtime::Runtime as CardamomRuntime;
use clap::{ArgEnum, Parser};
use generate_bags::generate_thresholds;
use selendra_runtime::Runtime as SelendraRuntime;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, ArgEnum)]
#[clap(rename_all = "PascalCase")]
enum Runtime {
	Cardamom,
	Selendra,
}

impl Runtime {
	fn generate_thresholds_fn(
		&self,
	) -> Box<dyn FnOnce(usize, &Path, u128, u128) -> Result<(), std::io::Error>> {
		match self {
			Runtime::Cardamom => Box::new(generate_thresholds::<CardamomRuntime>),
			Runtime::Selendra => Box::new(generate_thresholds::<SelendraRuntime>),
		}
	}
}

#[derive(Debug, Parser)]
struct Opt {
	/// How many bags to generate.
	#[clap(long, default_value = "200")]
	n_bags: usize,

	/// Which runtime to generate.
	#[clap(long, ignore_case = true, arg_enum, default_value = "Selendra")]
	runtime: Runtime,

	/// Where to write the output.
	output: PathBuf,

	/// The total issuance of the native currency.
	#[clap(short, long)]
	total_issuance: u128,

	/// The minimum account balance (i.e. existential deposit) for the native currency.
	#[clap(short, long)]
	minimum_balance: u128,
}

fn main() -> Result<(), std::io::Error> {
	let Opt { n_bags, output, runtime, total_issuance, minimum_balance } = Opt::parse();

	runtime.generate_thresholds_fn()(n_bags, &output, total_issuance, minimum_balance)
}
