// Copyright 2022 SmallWorld Selendra (Kh).
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

//! Collator for the `Undying` test parachain.

use sc_cli::{Error as SubstrateCliError, Role, SubstrateCli};
use selendra_cli::{Error, Result};
use selendra_node_primitives::CollationGenerationConfig;
use selendra_node_subsystem::messages::{CollationGenerationMessage, CollatorProtocolMessage};
use selendra_primitives::v1::Id as ParaId;
use sp_core::hexdisplay::HexDisplay;
use test_parachain_undying_collator::Collator;

mod cli;
use cli::Cli;

fn main() -> Result<()> {
	let cli = Cli::from_args();

	match cli.subcommand {
		Some(cli::Subcommand::ExportGenesisState(params)) => {
			// `pov_size` and `pvf_complexity` need to match the ones that we start the collator
			// with.
			let collator = Collator::new(params.pov_size, params.pvf_complexity);
			println!("0x{:?}", HexDisplay::from(&collator.genesis_head()));

			Ok::<_, Error>(())
		},
		Some(cli::Subcommand::ExportGenesisWasm(_params)) => {
			// We pass some dummy values for `pov_size` and `pvf_complexity` as these don't
			// matter for `wasm` export.
			println!("0x{:?}", HexDisplay::from(&Collator::default().validation_code()));

			Ok(())
		},
		None => {
			let runner = cli.create_runner(&cli.run.base).map_err(|e| {
				SubstrateCliError::Application(
					Box::new(e) as Box<(dyn 'static + Send + Sync + std::error::Error)>
				)
			})?;

			runner.run_node_until_exit(|config| async move {
				let role = config.role.clone();

				match role {
					Role::Light => Err("Light client not supported".into()),
					_ => {
						let collator = Collator::new(cli.run.pov_size, cli.run.pvf_complexity);

						let full_node = selendra_service::build_full(
							config,
							selendra_service::IsCollator::Yes(collator.collator_key()),
							None,
							true,
							None,
							None,
							false,
							selendra_service::RealOverseerGen,
						)
						.map_err(|e| e.to_string())?;
						let mut overseer_handle = full_node
							.overseer_handle
							.expect("Overseer handle should be initialized for collators");

						let genesis_head_hex =
							format!("0x{:?}", HexDisplay::from(&collator.genesis_head()));
						let validation_code_hex =
							format!("0x{:?}", HexDisplay::from(&collator.validation_code()));

						let para_id = ParaId::from(cli.run.parachain_id);

						log::info!("Running `Undying` collator for parachain id: {}", para_id);
						log::info!("Genesis state: {}", genesis_head_hex);
						log::info!("Validation code: {}", validation_code_hex);

						let config = CollationGenerationConfig {
							key: collator.collator_key(),
							collator: collator
								.create_collation_function(full_node.task_manager.spawn_handle()),
							para_id,
						};
						overseer_handle
							.send_msg(CollationGenerationMessage::Initialize(config), "Collator")
							.await;

						overseer_handle
							.send_msg(CollatorProtocolMessage::CollateOn(para_id), "Collator")
							.await;

						Ok(full_node.task_manager)
					},
				}
			})
		},
	}?;
	Ok(())
}
