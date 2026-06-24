//! Benchmarking setup for the Civitas node.

#[cfg(feature = "runtime-benchmarks")]
use {
    civitas_runtime::RuntimeApi, sc_executor::NativeElseWasmExecutor, sp_io::SubstrateHostFunctions,
};

/// Inherent benchmarking data.
pub fn inherent_benchmark_data() -> Result<Vec<u8>, String> {
    Ok(Vec::new())
}

/// The benchmarking executor.
#[cfg(feature = "runtime-benchmarks")]
pub type ExecutorDispatch = NativeElseWasmExecutor<RuntimeApi, SubstrateHostFunctions>;

/// Extra benchmarking parameters exposed to the CLI.
#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkParams {
    /// The database configuration.
    pub db: sc_service::config::DatabaseSource,
    /// Whether storage caching is enabled.
    pub storage: Option<sc_client_api::StorageProviderType>,
    /// The timeout for loading the runtime WASM code.
    pub code_load_timeout: Option<std::time::Duration>,
    /// Whether to benchmark the WASM or native runtime.
    pub wasm: bool,
}
