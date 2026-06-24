#![allow(clippy::result_large_err)]
#![allow(dead_code)]
//! Civitas node main entrypoint.

mod benchmarking;
mod chain_spec;
mod cli;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
