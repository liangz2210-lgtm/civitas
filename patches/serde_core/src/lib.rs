//! Shim serde_core that re-exports serde.
//!
//! serde_core 1.0.228 is broken on wasm32v1-none (missing #![no_std]).
//! This shim replaces it by re-exporting serde 1.0.219, which has proper
//! no_std support and contains all the same traits.

#![cfg_attr(not(feature = "std"), no_std)]

// Re-export everything from serde. serde_core is a subset of serde's API
// (traits without derive), so this is a superset and fully compatible.
pub use serde::*;
