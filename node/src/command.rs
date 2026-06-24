//! CLI command execution.

use crate::{chain_spec, cli::{Cli, Subcommand}, service};
use civitas_runtime::Block;
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Civitas Node".into()
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
		"https://github.com/liangz2210-lgtm/civitas/issues".into()
	}

	fn copyright_start_year() -> i32 {
		2024
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"dev" => Box::new(chain_spec::development_config()?),
			"" | "local" => Box::new(chain_spec::local_testnet_config()?),
			path =>
				Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
		})
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					service::new_partial(&config)?;
				let aux_revert = Box::new(|client, _backend, blocks| {
					sc_consensus_grandpa::revert(client, blocks)?;
					Ok(())
				});
				Ok((cmd.run(client, backend, Some(aux_revert)), task_manager))
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				#[cfg(feature = "runtime-benchmarks")]
				{
					let sc_service::PartialComponents { client, .. } =
						service::new_partial(&config)?;
					use frame_benchmarking_cli::BenchmarkCmd;
					match cmd {
						BenchmarkCmd::Pallet(cmd) => {
							let params =
								frame_benchmarking_cli::PalletBenchmarkParamsBuilder::default()
									.wasm(true)
									.build()?;
							cmd.run::<Block, ()>(params)
						},
						BenchmarkCmd::Block(cmd) => cmd.run(client),
						BenchmarkCmd::Storage(cmd) => {
							let db = config.database.path().ok_or_else(|| {
								"Database path is required".to_string()
							})?;
							cmd.run(client, db, &crate::benchmarking::inherent_benchmark_data()?)
						},
						BenchmarkCmd::Overhead(cmd) => {
							let db = config.database.path().ok_or_else(|| {
								"Database path is required".to_string()
							})?;
							cmd.run(
								client,
								db,
								&crate::benchmarking::inherent_benchmark_data()?,
								std::time::Duration::from_secs(60),
							)
						},
						BenchmarkCmd::Machine(cmd) => cmd.run(),
						_ => Err("Benchmarking sub-command not supported".into()),
					}
				}

				#[cfg(not(feature = "runtime-benchmarks"))]
				{
					let _ = config;
					Err("Benchmarking was not enabled. Re-run with `--features runtime-benchmarks`.".into())
				}
			})
		},
		Some(Subcommand::ChainInfo(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run::<Block>(&config))
		},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				match config.network.network_backend {
					sc_network::config::NetworkBackendType::Libp2p =>
						service::new_full::<sc_network::NetworkWorker<
							civitas_runtime::opaque::Block,
							<civitas_runtime::opaque::Block as sp_runtime::traits::Block>::Hash,
						>>(config)
						.map_err(sc_cli::Error::Service),
					sc_network::config::NetworkBackendType::Litep2p =>
						service::new_full::<sc_network::Litep2pNetworkBackend>(config)
						.map_err(sc_cli::Error::Service),
				}
			})
		},
	}
}
