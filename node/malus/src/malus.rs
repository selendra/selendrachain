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

//! A malus or nemesis node launch code.

use clap::Parser;
use color_eyre::eyre;
use selendra_cli::{Cli, RunCmd};

pub(crate) mod interceptor;
pub(crate) mod shared;

mod variants;

use variants::*;

/// Define the different variants of behavior.
#[derive(Debug, Parser)]
#[clap(about = "Malus - the nemesis of polkadot.", version)]
#[clap(rename_all = "kebab-case")]
enum NemesisVariant {
	/// Suggest a candidate with an invalid proof of validity.
	SuggestGarbageCandidate(RunCmd),
	/// Back a candidate with a specifically crafted proof of validity.
	BackGarbageCandidate(RunCmd),
	/// Delayed disputing of ancestors that are perfectly fine.
	DisputeAncestor(RunCmd),

	#[allow(missing_docs)]
	#[clap(name = "prepare-worker", hide = true)]
	PvfPrepareWorker(selendra_cli::ValidationWorkerCommand),

	#[allow(missing_docs)]
	#[clap(name = "execute-worker", hide = true)]
	PvfExecuteWorker(selendra_cli::ValidationWorkerCommand),
}

#[derive(Debug, Parser)]
#[allow(missing_docs)]
struct MalusCli {
	#[clap(subcommand)]
	pub variant: NemesisVariant,
}

fn run_cmd(run: RunCmd) -> Cli {
	Cli { subcommand: None, run }
}

impl MalusCli {
	/// Launch a malus node.
	fn launch(self) -> eyre::Result<()> {
		match self.variant {
			NemesisVariant::BackGarbageCandidate(cmd) =>
				selendra_cli::run_node(run_cmd(cmd), BackGarbageCandidate)?,
			NemesisVariant::SuggestGarbageCandidate(cmd) =>
				selendra_cli::run_node(run_cmd(cmd), SuggestGarbageCandidate)?,
			NemesisVariant::DisputeAncestor(cmd) =>
				selendra_cli::run_node(run_cmd(cmd), DisputeValidCandidates)?,
			NemesisVariant::PvfPrepareWorker(cmd) => {
				#[cfg(target_os = "android")]
				{
					return Err("PVF preparation workers are not supported under this platform")
						.into()
				}

				#[cfg(not(target_os = "android"))]
				{
					selendra_node_core_pvf::prepare_worker_entrypoint(&cmd.socket_path);
				}
			},
			NemesisVariant::PvfExecuteWorker(cmd) => {
				#[cfg(target_os = "android")]
				{
					return Err("PVF execution workers are not supported under this platform").into()
				}

				#[cfg(not(target_os = "android"))]
				{
					selendra_node_core_pvf::execute_worker_entrypoint(&cmd.socket_path);
				}
			},
		}
		Ok(())
	}
}

fn main() -> eyre::Result<()> {
	color_eyre::install()?;
	let cli = MalusCli::parse();
	cli.launch()?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn subcommand_works() {
		let cli = MalusCli::try_parse_from(IntoIterator::into_iter([
			"malus",
			"dispute-ancestor",
			"--bob",
		]))
		.unwrap();
		assert_matches::assert_matches!(cli, MalusCli {
			variant: NemesisVariant::DisputeAncestor(run),
			..
		} => {
			assert!(run.base.bob);
		});
	}
}
