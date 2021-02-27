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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IndracoreService(#[from] service::Error),

    #[error(transparent)]
    SubstrateCli(#[from] sc_cli::Error),

    #[error(transparent)]
    SubstrateService(#[from] sc_service::Error),

    #[error("Other: {0}")]
    Other(String),
}

impl std::convert::From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

type Result<T> = std::result::Result<T, Error>;

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Selendra Indracore".into()
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
        "https://github.com/paritytech/indracore/issues/new".into()
    }

    fn copyright_start_year() -> i32 {
        2017
    }

    fn executable_name() -> String {
        "indracore".into()
    }

    fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
        let spec = match id {
            "indracore-dev" | "dev" => {
                Box::new(service::chain_spec::indracore_development_config()?)
            }
            "indracore-local" | "" => {
                Box::new(service::chain_spec::indracore_local_testnet_config()?)
            }
            "indracore-staging" => {
                Box::new(service::chain_spec::indracore_staging_testnet_config()?)
            }
            "indracore" => Box::new(service::chain_spec::indracore_config()?),
            path => Box::new(service::IndracoreChainSpec::from_json_file(
                std::path::PathBuf::from(path),
            )?),
        };
        Ok(spec)
    }

    fn native_runtime_version(_spec: &Box<dyn service::ChainSpec>) -> &'static RuntimeVersion {
        &service::indracore_runtime::VERSION
    }
}

fn set_default_ss58_version(_spec: &Box<dyn service::ChainSpec>) {
    use sp_core::crypto::Ss58AddressFormat;
    let ss58_version = Ss58AddressFormat::SubstraTeeAccount;
    sp_core::crypto::set_default_ss58_version(ss58_version);
}

/// Parses indracore specific CLI arguments and run the service.
pub fn run() -> Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        None => {
            let runner = cli.create_runner(&cli.run.base).map_err(Error::from)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            let grandpa_pause = if cli.run.grandpa_pause.is_empty() {
                None
            } else {
                Some((cli.run.grandpa_pause[0], cli.run.grandpa_pause[1]))
            };

            let jaeger_agent = cli.run.jaeger_agent;

            runner.run_node_until_exit(move |config| async move {
                let role = config.role.clone();

                let task_manager = match role {
                    Role::Light => {
                        service::build_light(config).map(|(task_manager, _, _)| task_manager)
                    }
                    _ => service::build_full(
                        config,
                        service::IsCollator::No,
                        grandpa_pause,
                        jaeger_agent,
                    )
                    .map(|full| full.task_manager),
                }?;
                Ok::<_, Error>(task_manager)
            })
        }
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|config| cmd.run(config.chain_spec, config.network))?)
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd).map_err(Error::SubstrateCli)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) =
                    service::new_chain_ops(&mut config, None)?;
                Ok((
                    cmd.run(client, import_queue).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            Ok(runner.async_run(|mut config| {
                let (client, _, _, task_manager) =
                    service::new_chain_ops(&mut config, None).map_err(Error::IndracoreService)?;
                Ok((
                    cmd.run(client, config.database)
                        .map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?)
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            Ok(runner.async_run(|mut config| {
                let (client, _, _, task_manager) = service::new_chain_ops(&mut config, None)?;
                Ok((
                    cmd.run(client, config.chain_spec)
                        .map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?)
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            Ok(runner.async_run(|mut config| {
                let (client, _, import_queue, task_manager) =
                    service::new_chain_ops(&mut config, None)?;
                Ok((
                    cmd.run(client, import_queue).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?)
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            Ok(runner.sync_run(|config| cmd.run(config.database))?)
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            Ok(runner.async_run(|mut config| {
                let (client, backend, _, task_manager) = service::new_chain_ops(&mut config, None)?;
                Ok((
                    cmd.run(client, backend).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?)
        }
        Some(Subcommand::ValidationWorker(cmd)) => {
            let mut builder = sc_cli::LoggerBuilder::new("");
            builder.with_colors(false);
            let _ = builder.init();

            if cfg!(feature = "browser") || cfg!(target_os = "android") {
                Err(sc_cli::Error::Input("Cannot run validation worker in browser".into()).into())
            } else {
                #[cfg(not(any(target_os = "android", feature = "browser")))]
                indracore_parachain::wasm_executor::run_worker(
                    &cmd.mem_id,
                    Some(cmd.cache_base_path.clone()),
                )?;
                Ok(())
            }
        }
        Some(Subcommand::Benchmark(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;

            set_default_ss58_version(chain_spec);

            Ok(runner.sync_run(|config| {
                cmd.run::<service::indracore_runtime::Block, service::IndracoreExecutor>(config)
                    .map_err(|e| Error::SubstrateCli(e))
            })?)
        }
        Some(Subcommand::Key(cmd)) => Ok(cmd.run(&cli)?),

        #[cfg(feature = "try-runtime")]
        Some(Subcommand::TryRuntime(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            let chain_spec = &runner.config().chain_spec;
            set_default_ss58_version(chain_spec);

            runner.async_run(|config| {
                use sc_service::TaskManager;
                let registry = config.prometheus_config.as_ref().map(|cfg| &cfg.registry);
                let task_manager = TaskManager::new(config.task_executor.clone(), registry)
                    .map_err(|e| Error::SubstrateService(sc_service::Error::Prometheus(e)))?;

                Ok((
                    cmd.run::<service::indracore_runtime::Block, service::IndracoreExecutor>(
                        config,
                    )
                    .map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })
        }
    }?;
    Ok(())
}
