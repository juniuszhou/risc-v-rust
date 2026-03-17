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
    load_raw_bytecode("bytecode/polkavm/arithmetic.polkavm")
}

/// Load EVM bytecode from file (hex-encoded, e.g. "6080604052...")
/// Returns (init_bytecode, runtime_bytecode). For deployment use init; for direct deploy use runtime.
pub fn load_evm_bytecode() -> Result<(Vec<u8>, Vec<u8>)> {
    let hex_str = std::fs::read_to_string("bytecode/evm/arithmetic.bin")
        .map_err(|e| anyhow!("Failed to read EVM bytecode: {}", e))?;
    let hex_str = hex_str.trim().strip_prefix("0x").unwrap_or(hex_str.trim());
    let full = alloy_primitives::hex::decode(hex_str)
        .map_err(|e| anyhow!("Invalid EVM bytecode hex: {}", e))?;
    // Solidity deployment bytecode = init + runtime. Init ends with RETURN (0xf3).
    // Runtime typically starts with 0x60 0x20 0x01 (PUSH1 0x20 ADD) or 0x5b (JUMPDEST).
    let init_len = full
        .iter()
        .position(|&b| b == 0xf3)
        .map(|i| i + 1)
        .unwrap_or(0);
    // If split looks wrong (runtime doesn't start with valid opcode), try alternate offset
    let init_len = if init_len > 0 && init_len < full.len() {
        let first = full[init_len];
        if first == 0xf3 || first == 0x90 {
            init_len + 1
        } else {
            init_len
        }
    } else {
        init_len
    };
    let (init, runtime) = if full.len() > init_len && init_len > 0 {
        full.split_at(init_len)
    } else {
        (full.as_slice(), &[][..])
    };
    Ok((init.to_vec(), runtime.to_vec()))
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
    let (init_bytecode, runtime_bytecode) = match load_evm_bytecode() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Skipping revm arithmetic benchmark: {}", e);
            return;
        }
    };

    let mut executor = RevmExecutor::new();
    if runtime_bytecode.is_empty() {
        executor
            .deploy_create(init_bytecode)
            .expect("deploy_create");
    } else {
        executor.deploy(runtime_bytecode).expect("deploy runtime");
    }
    let calldata = arithmetic::encode_compute(U256::from(12345), U256::from(6789));

    let result = match executor.call(calldata) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping revm arithmetic benchmark: call failed: {}", e);
            return;
        }
    };
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
