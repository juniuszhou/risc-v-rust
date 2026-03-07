//! PolkaVM host: load and execute RISC-V bytecode.
//!
//! Usage: pvm <path-to-bytecode.polkavm>
//!
//! Example: Build the fibonacci guest, then:
//!   cd ../fibonacci-rust && ./build.sh
//!   cargo run -- ../fibonacci-rust/fibonacci.polkavm

mod evm_runner;
mod polkavm_runner;
use alloy_primitives::{Bytes, U256};
use alloy_sol_types::{sol, SolCall};
use anyhow::{anyhow, Result};
use evm_runner::RevmExecutor;
use polkavm_runner::PolkaVmExecutor;

/// Load PolkaVM bytecode from file
pub fn load_polkavm_bytecode() -> Result<Vec<u8>> {
    let path = format!("bytecode/polkavm/arithmetic.polkavm");
    load_raw_bytecode(&path)
}

/// Load raw bytecode from a file
fn load_raw_bytecode(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| anyhow!("Failed to read {}: {}", path, e))
}

sol! {
    interface IArithmetic {
        function compute(uint256 a, uint256 b) external pure returns (uint256);
        function computeMany(uint256 iterations) external pure returns (uint256);
    }
}

pub mod arithmetic {
    use super::*;

    pub fn encode_compute(a: U256, b: U256) -> Bytes {
        IArithmetic::computeCall { a, b }.abi_encode().into()
    }

    pub fn encode_compute_many(iterations: U256) -> Bytes {
        IArithmetic::computeManyCall { iterations }
            .abi_encode()
            .into()
    }
}

/// Benchmark arithmetic operations on revm
fn bench_revm_arithmetic() {
    // Load bytecode
    let bytecode = match load_polkavm_bytecode() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Skipping revm arithmetic benchmark: {}", e);
            return;
        }
    };

    let mut executor = RevmExecutor::new();
    executor.deploy(bytecode.clone()).unwrap();
    let calldata = arithmetic::encode_compute(U256::from(12345), U256::from(6789));

    let result = executor.call(calldata).unwrap();
    println!("Result: {:?}", result);
}

/// Benchmark arithmetic operations on revm
fn bench_pvm_arithmetic() {
    // Load bytecode
    let bytecode = match load_polkavm_bytecode() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Skipping revm arithmetic benchmark: {}", e);
            return;
        }
    };

    let mut executor = match PolkaVmExecutor::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to create PolkaVM executor: {}", e);
            return;
        }
    };

    let module = match executor.load_module(&bytecode) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load PolkaVM module: {}", e);
            return;
        }
    };

    let calldata = arithmetic::encode_compute(U256::from(12345), U256::from(6789));

    let result = executor.call_with_data(&module, &calldata).unwrap();

    println!("Result: {:?}", result);
}

// sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
fn main() -> Result<(), Box<dyn std::error::Error>> {
    bench_pvm_arithmetic();

    Ok(())
}
