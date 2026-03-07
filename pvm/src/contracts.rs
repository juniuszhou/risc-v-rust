use alloy_primitives::{Bytes, U256};
use alloy_sol_types::{sol, SolCall};
use anyhow::{anyhow, Result};
use std::path::Path;

/// Load PolkaVM bytecode from file
pub fn load_polkavm_bytecode() -> Result<Vec<u8>> {
    let path = format!("bytecode/polkavm/arithmetic.polkavm");
    load_raw_bytecode(&path)
}

/// Load raw bytecode from a file
fn load_raw_bytecode(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| anyhow!("Failed to read {}: {}", path, e))
}
