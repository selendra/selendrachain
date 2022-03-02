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

use crate::cli::{Cli, Subcommand};
use futures::future::TryFutureExt;
use sc_cli::{Role, RuntimeVersion, SubstrateCli};
use service::{self, IdentifyVariant};
use sp_core::crypto::Ss58AddressFormat;

pub use crate::error::Error;
pub use selendra_performance_test::PerfCheckError;

impl std::convert::From<String> for Error {
	fn from(s: String) -> Self {
		Self::Other(s)
	}
}

type Result<T> = std::result::Result<T, Error>;

fn get_exec_name() -> Option<String> {
	std::env::current_exe()
		.ok()
		.and_then(|pb| pb.file_name().map(|s| s.to_os_string()))
		.and_then(|s| s.into_string().ok())
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Selendra SelendraChain".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/selendra/selendrachain/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2020
	}

	fn executable_name() -> String {
		"selendra".into()
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		let id = if id == "" {
			let n = get_exec_name().unwrap_or_default();
			["selendra", "cardamom"]
				.iter()
				.cloned()
				.find(|&chain| n.starts_with(chain))
				.unwrap_or("selendra")
		} else {
			id
		};
		Ok(match id {
			"selendra" => Box::new(service::chain_spec::selendra_config()?),
			#[cfg(feature = "selendra-native")]
			"dev" | "selendra-dev" => Box::new(service::chain_spec::selendra_development_config()?),
			#[cfg(feature = "selendra-native")]
			"selendra-local" => Box::new(service::chain_spec::selendra_local_testnet_config()?),
			#[cfg(feature = "selendra-native")]
			"selendra-staging" => Box::new(service::chain_spec::selendra_staging_testnet_config()?),
			#[cfg(not(feature = "selendra-native"))]
			name if name.starts_with("selendra-") && !name.ends_with(".json") =>
				Err(format!("`{}` only supported with `selendra-native` feature enabled.", name))?,
			"cardamom" => Box::new(service::chain_spec::cardamom_config()?),
			#[cfg(feature = "cardamom-native")]
			"cardamom-dev" => Box::new(service::chain_spec::cardamom_development_config()?),
			#[cfg(feature = "cardamom-native")]
			"cardamom-local" => Box::new(service::chain_spec::cardamom_local_testnet_config()?),
			#[cfg(feature = "cardamom-native")]
			"cardamom-staging" => Box::new(service::chain_spec::cardamom_staging_testnet_config()?),
			#[cfg(not(feature = "cardamom-native"))]
			name if name.starts_with("cardamom-") && !name.ends_with(".json") =>
				Err(format!("`{}` only supported with `cardamom-native` feature enabled.", name))?,
			path => {
				let path = std::path::PathBuf::from(path);

				let chain_spec = Box::new(service::SelendraChainSpec::from_json_file(path.clone())?)
					as Box<dyn service::ChainSpec>;

				if self.run.force_cardamom || chain_spec.is_cardamom() {
					Box::new(service::CardamomChainSpec::from_json_file(path)?)
				} else {
					chain_spec
				}
			},
		})
	}

	fn native_runtime_version(spec: &Box<dyn service::ChainSpec>) -> &'static RuntimeVersion {
		#[cfg(feature = "cardamom-native")]
		if spec.is_cardamom() {
			return &service::cardamom_runtime::VERSION
		}

		#[cfg(not(all(feature = "cardamom-native",)))]
		let _ = spec;

		#[cfg(feature = "selendra-native")]
		{
			return &service::selendra_runtime::VERSION
		}

		#[cfg(not(feature = "selendra-native"))]
		panic!("No runtime feature (selendra, cardamom) is enabled")
	}
}

fn set_default_ss58_version() {
	let ss58_version = Ss58AddressFormat::custom(972);
	sp_core::crypto::set_default_ss58_version(ss58_version);
}

const DEV_ONLY_ERROR_PATTERN: &'static str =
	"can only use subcommand with --chain [selendra-dev, cardamom-dev, -dev], got ";

fn ensure_dev(spec: &Box<dyn service::ChainSpec>) -> std::result::Result<(), String> {
	if spec.is_dev() {
		Ok(())
	} else {
		Err(format!("{}{}", DEV_ONLY_ERROR_PATTERN, spec.id()))
	}
}

/// Runs performance checks.
/// Should only be used in release build since the check would take too much time otherwise.
fn host_perf_check() -> Result<()> {
	#[cfg(not(build_type = "release"))]
	{
		Err(PerfCheckError::WrongBuildType.into())
	}
	#[cfg(build_type = "release")]
	{
		crate::host_perf_check::host_perf_check()?;
		Ok(())
	}
}

fn run_node_inner<F>(
	cli: Cli,
	overseer_gen: impl service::OverseerGen,
	logger_hook: F,
) -> Result<()>
where
	F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
{
	let runner = cli
		.create_runner_with_logger_hook::<sc_cli::RunCmd, F>(&cli.run.base, logger_hook)
		.map_err(Error::from)?;

	set_default_ss58_version();

	let grandpa_pause = if cli.run.grandpa_pause.is_empty() {
		None
	} else {
		Some((cli.run.grandpa_pause[0], cli.run.grandpa_pause[1]))
	};

	let jaeger_agent = cli.run.jaeger_agent;

	runner.run_node_until_exit(move |config| async move {
		let role = config.role.clone();

		match role {
			Role::Light => Err(Error::Other("Light client not enabled".into())),
			_ => service::build_full(
				config,
				service::IsCollator::No,
				grandpa_pause,
				cli.run.beefy,
				jaeger_agent,
				None,
				false,
				overseer_gen,
			)
			.map(|full| full.task_manager)
			.map_err(Into::into),
		}
	})
}

/// Parses selendra specific CLI arguments and run the service.
pub fn run() -> Result<()> {
	let cli: Cli = Cli::from_args();

	match &cli.subcommand {
		None => run_node_inner(cli, service::RealOverseerGen, selendra_node_metrics::logger_hook()),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			Ok(runner.sync_run(|config| cmd.run(config.chain_spec, config.network))?)
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd).map_err(Error::SubstrateCli)?;

			set_default_ss58_version();

			runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) =
					service::new_chain_ops(&mut config, None)?;
				Ok((cmd.run(client, import_queue).map_err(Error::SubstrateCli), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			set_default_ss58_version();

			Ok(runner.async_run(|mut config| {
				let (client, _, _, task_manager) =
					service::new_chain_ops(&mut config, None).map_err(Error::SelendraService)?;
				Ok((cmd.run(client, config.database).map_err(Error::SubstrateCli), task_manager))
			})?)
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			set_default_ss58_version();

			Ok(runner.async_run(|mut config| {
				let (client, _, _, task_manager) = service::new_chain_ops(&mut config, None)?;
				Ok((cmd.run(client, config.chain_spec).map_err(Error::SubstrateCli), task_manager))
			})?)
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			set_default_ss58_version();

			Ok(runner.async_run(|mut config| {
				let (client, _, import_queue, task_manager) =
					service::new_chain_ops(&mut config, None)?;
				Ok((cmd.run(client, import_queue).map_err(Error::SubstrateCli), task_manager))
			})?)
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			Ok(runner.sync_run(|config| cmd.run(config.database))?)
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			set_default_ss58_version();

			Ok(runner.async_run(|mut config| {
				let (client, backend, _, task_manager) = service::new_chain_ops(&mut config, None)?;
				Ok((cmd.run(client, backend).map_err(Error::SubstrateCli), task_manager))
			})?)
		},
		Some(Subcommand::PvfPrepareWorker(cmd)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_colors(false);
			let _ = builder.init();

			#[cfg(target_os = "android")]
			{
				return Err(sc_cli::Error::Input(
					"PVF preparation workers are not supported under this platform".into(),
				)
				.into())
			}

			#[cfg(not(target_os = "android"))]
			{
				selendra_node_core_pvf::prepare_worker_entrypoint(&cmd.socket_path);
				Ok(())
			}
		},
		Some(Subcommand::PvfExecuteWorker(cmd)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_colors(false);
			let _ = builder.init();

			#[cfg(target_os = "android")]
			{
				return Err(sc_cli::Error::Input(
					"PVF execution workers are not supported under this platform".into(),
				)
				.into())
			}

			#[cfg(not(target_os = "android"))]
			{
				selendra_node_core_pvf::execute_worker_entrypoint(&cmd.socket_path);
				Ok(())
			}
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			set_default_ss58_version();

			ensure_dev(chain_spec).map_err(Error::Other)?;

			#[cfg(feature = "cardamom-native")]
			if chain_spec.is_cardamom() {
				return Ok(runner.sync_run(|config| {
					cmd.run::<service::cardamom_runtime::Block, service::CardamomExecutorDispatch>(
						config,
					)
					.map_err(|e| Error::SubstrateCli(e))
				})?)
			}

			// else we assume it is selendra.
			#[cfg(feature = "selendra-native")]
			{
				return Ok(runner.sync_run(|config| {
					cmd.run::<service::selendra_runtime::Block, service::SelendraExecutorDispatch>(
						config,
					)
					.map_err(|e| Error::SubstrateCli(e))
				})?)
			}
			#[cfg(not(feature = "selendra-native"))]
			panic!("No runtime feature (selendra, cardamom,) is enabled")
		},
		Some(Subcommand::HostPerfCheck) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_colors(true);
			builder.init()?;

			host_perf_check()
		},
		Some(Subcommand::Key(cmd)) => Ok(cmd.run(&cli)?),
		#[cfg(feature = "try-runtime")]
		Some(Subcommand::TryRuntime(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			set_default_ss58_version();

			use sc_service::TaskManager;
			let registry = &runner.config().prometheus_config.as_ref().map(|cfg| &cfg.registry);
			let task_manager = TaskManager::new(runner.config().tokio_handle.clone(), *registry)
				.map_err(|e| Error::SubstrateService(sc_service::Error::Prometheus(e)))?;

			ensure_dev(chain_spec).map_err(Error::Other)?;

			#[cfg(feature = "cardamom-native")]
			if chain_spec.is_cardamom() {
				return runner.async_run(|config| {
					Ok((
						cmd.run::<service::cardamom_runtime::Block, service::CardamomExecutorDispatch>(
							config,
						)
						.map_err(Error::SubstrateCli),
						task_manager,
					))
				})
			}
			// else we assume it is selendra.
			#[cfg(feature = "selendra-native")]
			{
				return runner.async_run(|config| {
					Ok((
						cmd.run::<service::selendra_runtime::Block, service::SelendraExecutorDispatch>(
							config,
						)
						.map_err(Error::SubstrateCli),
						task_manager,
					))
				})
			}
			#[cfg(not(feature = "selendra-native"))]
			panic!("No runtime feature (selendra, cardamom) is enabled")
		},
		#[cfg(not(feature = "try-runtime"))]
		Some(Subcommand::TryRuntime) => Err(Error::Other(
			"TryRuntime wasn't enabled when building the node. \
				You can enable it with `--features try-runtime`."
				.into(),
		)
		.into()),
	}?;
	Ok(())
}
