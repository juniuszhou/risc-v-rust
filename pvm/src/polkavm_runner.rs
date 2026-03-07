//! PolkaVM Runner - PolkaVM executor wrapper for benchmarking
//!
//! This module handles execution of Solidity contracts compiled with revive/resolc.
//! The execution model matches pallet-revive: using RawInstance with an interrupt-based
//! syscall dispatch loop instead of the Linker abstraction.

use alloy_primitives::{keccak256, Bytes};
use anyhow::{anyhow, Result};
use polkavm::{
    BackendKind, Config, Engine, GasMeteringKind, InterruptKind, Module, ModuleConfig,
    ProgramBlob, Reg,
};
use std::collections::HashMap;

/// Syscall identifiers for fast dispatch (avoids string comparison per call)
#[derive(Clone, Copy, Debug)]
pub enum SyscallId {
    CallDataSize,
    CallDataLoad,
    CallDataCopy,
    SealReturn,
    ConsumeAllGas,
    ValueTransferred,
    SetImmutableData,
    GetImmutableData,
    GetStorageOrZero,
    SetStorageOrClear,
    HashKeccak256,
    Address,
    Caller,
    Balance,
    BlockNumber,
    ReturnDataSize,
    RefTimeLeft,
    Unknown,
}

/// Gas limit for PolkaVM execution (300M to support large iteration counts)
pub const GAS_LIMIT: i64 = 300_000_000;

/// Outcome of contract execution
enum ExecOutcome {
    /// Contract returned via seal_return
    Return { flags: u32, data: Vec<u8> },
    /// Contract trapped (unexpected termination)
    Trap,
    /// Ran out of gas
    OutOfGas,
    /// Finished normally (no explicit return)
    Finished,
}

/// Result of handling a single syscall
enum SyscallResult {
    /// Continue execution (no return value for caller)
    Continue,
    /// Continue execution, writing a return value to register A0
    ReturnValue(u64),
    /// Terminate execution with seal_return data
    Terminate { flags: u32, data: Vec<u8> },
}

/// Map syscall symbol bytes to SyscallId for fast dispatch
fn symbol_to_syscall_id(symbol: &[u8]) -> SyscallId {
    match symbol {
        b"call_data_size" => SyscallId::CallDataSize,
        b"call_data_load" => SyscallId::CallDataLoad,
        b"call_data_copy" => SyscallId::CallDataCopy,
        b"seal_return" => SyscallId::SealReturn,
        b"consume_all_gas" => SyscallId::ConsumeAllGas,
        b"value_transferred" => SyscallId::ValueTransferred,
        b"set_immutable_data" => SyscallId::SetImmutableData,
        b"get_immutable_data" => SyscallId::GetImmutableData,
        b"get_storage_or_zero" => SyscallId::GetStorageOrZero,
        b"set_storage_or_clear" => SyscallId::SetStorageOrClear,
        b"hash_keccak_256" => SyscallId::HashKeccak256,
        b"address" => SyscallId::Address,
        b"caller" => SyscallId::Caller,
        b"balance" => SyscallId::Balance,
        b"block_number" => SyscallId::BlockNumber,
        b"return_data_size" => SyscallId::ReturnDataSize,
        b"ref_time_left" => SyscallId::RefTimeLeft,
        _ => SyscallId::Unknown,
    }
}

/// PolkaVM executor wrapper for benchmarking revive-compiled contracts.
///
/// Uses the same interrupt-based execution model as pallet-revive:
/// - `RawInstance` for low-level PolkaVM control
/// - `instance.run()` loop with `Ecalli` interrupt handling
/// - Register-based parameter passing (A0..A5)
///
/// Performance optimizations:
/// - Compiler backend (not interpreter) for native speed
/// - Sandbox disabled for benchmarking (no IPC overhead)
/// - Pre-built syscall dispatch table (no string comparison per call)
pub struct PolkaVmExecutor {
    engine: Engine,
    /// Persistent storage for stateful contracts (storage benchmarks)
    storage: HashMap<[u8; 32], [u8; 32]>,
}

/// Pre-computed syscall dispatch table for a module
/// Maps import index -> SyscallId for O(1) lookup during execution
pub struct SyscallDispatchTable {
    table: Vec<SyscallId>,
}

impl SyscallDispatchTable {
    /// Build dispatch table from module imports
    pub fn from_module(module: &Module) -> Self {
        let mut table = Vec::new();
        // Iterate over all import indices and build dispatch table
        let mut idx = 0u32;
        while let Some(import) = module.imports().get(idx) {
            table.push(symbol_to_syscall_id(import.as_bytes()));
            idx += 1;
        }
        Self { table }
    }

    /// Get syscall ID for an import index
    #[inline(always)]
    pub fn get(&self, idx: u32) -> SyscallId {
        self.table.get(idx as usize).copied().unwrap_or(SyscallId::Unknown)
    }
}

impl PolkaVmExecutor {
    /// Create a new PolkaVM executor with optimized configuration for benchmarking
    ///
    /// Configuration:
    /// - Compiler backend (not interpreter) for native execution speed
    /// - Sandbox disabled for benchmarking (removes IPC overhead)
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        // Use compiler backend for maximum performance
        config.set_backend(Some(BackendKind::Compiler));
        // Enable experimental features to allow disabling sandbox
        config.set_allow_experimental(true);
        // Disable sandbox for benchmarking - removes IPC overhead
        // WARNING: Only use this for benchmarking, not production!
        config.set_sandboxing_enabled(false);

        let engine = Engine::new(&config).map_err(|e| anyhow!("Engine error: {}", e))?;

        Ok(Self {
            engine,
            storage: HashMap::new(),
        })
    }

    /// Load a module from PolkaVM bytecode
    pub fn load_module(&self, bytecode: &[u8]) -> Result<Module> {
        let blob = ProgramBlob::parse(bytecode.into())
            .map_err(|e| anyhow!("Failed to parse program blob: {:?}", e))?;

        let mut module_config = ModuleConfig::default();
        module_config.set_gas_metering(Some(GasMeteringKind::Sync));

        let module = Module::from_blob(&self.engine, &module_config, blob)
            .map_err(|e| anyhow!("Failed to create module: {}", e))?;
        Ok(module)
    }

    /// Reset storage state (for use between benchmark iterations)
    pub fn reset_storage(&mut self) {
        self.storage.clear();
    }

    /// Print module imports/exports for debugging
    #[allow(dead_code)]
    pub fn print_module_info(&self, module: &Module) {
        eprintln!("Module exports:");
        for export in module.exports() {
            eprintln!(
                "  - {:?}",
                std::str::from_utf8(export.symbol().as_bytes())
            );
        }
    }

    /// Execute a contract call with the given calldata.
    ///
    /// Matches the pallet-revive execution model:
    /// 1. Find "call" export and get its program counter
    /// 2. Create RawInstance, set gas, prepare call
    /// 3. Run in a loop, handling Ecalli interrupts as syscalls
    /// 4. Return execution result with gas used
    ///
    /// Performance optimizations:
    /// - Pre-built syscall dispatch table (O(1) lookup, no string comparison)
    /// - Compiler backend with sandbox disabled (configured in new())
    pub fn call_with_data(
        &mut self,
        module: &Module,
        calldata: &[u8],
    ) -> Result<(i64, Bytes)> {
        // Build dispatch table once per call (could be cached per module for more perf)
        let dispatch_table = SyscallDispatchTable::from_module(module);
        self.call_with_dispatch_table(module, calldata, &dispatch_table)
    }

    /// Execute with a pre-built dispatch table (for repeated calls to same module)
    pub fn call_with_dispatch_table(
        &mut self,
        module: &Module,
        calldata: &[u8],
        dispatch_table: &SyscallDispatchTable,
    ) -> Result<(i64, Bytes)> {
        // Find "call" entry point by program counter (matching pallet-revive)
        let entry_pc = module
            .exports()
            .find(|e| e.symbol().as_bytes() == b"call")
            .ok_or_else(|| anyhow!("No 'call' export found in module"))?
            .program_counter();

        // Create raw instance (no Linker needed - we handle syscalls via interrupts)
        let mut instance = module
            .instantiate()
            .map_err(|e| anyhow!("Failed to instantiate module: {}", e))?;

        instance.set_gas(GAS_LIMIT);
        instance.prepare_call_untyped(entry_pc, &[]);

        // Execution loop - matches pallet-revive's PreparedCall::call()
        // Optimized: uses dispatch table for O(1) syscall lookup
        let outcome = loop {
            let interrupt = instance.run();
            match interrupt {
                Ok(InterruptKind::Finished) => break ExecOutcome::Finished,
                Ok(InterruptKind::Trap) => break ExecOutcome::Trap,
                Ok(InterruptKind::NotEnoughGas) => break ExecOutcome::OutOfGas,
                Ok(InterruptKind::Segfault(_)) => break ExecOutcome::Trap,
                Ok(InterruptKind::Step) => continue,
                Ok(InterruptKind::Ecalli(idx)) => {
                    // O(1) dispatch table lookup instead of string comparison
                    let syscall_id = dispatch_table.get(idx);

                    match self.handle_syscall_by_id(&mut instance, syscall_id, calldata) {
                        SyscallResult::Continue => continue,
                        SyscallResult::ReturnValue(val) => {
                            // Write return value to register A0 (matching pallet-revive)
                            instance.set_reg(Reg::A0, val);
                            continue;
                        }
                        SyscallResult::Terminate { flags, data } => {
                            break ExecOutcome::Return { flags, data };
                        }
                    }
                }
                Err(e) => return Err(anyhow!("PolkaVM execution error: {}", e)),
            }
        };

        let gas_used = GAS_LIMIT - instance.gas();

        match outcome {
            ExecOutcome::Return { flags, data } => {
                if flags & 1 != 0 {
                    // Revert flag set - contract reverted
                    Err(anyhow!(
                        "Contract reverted (flags=0x{:x}, {} bytes output)",
                        flags,
                        data.len()
                    ))
                } else {
                    Ok((gas_used, Bytes::from(data)))
                }
            }
            ExecOutcome::Finished => Ok((gas_used, Bytes::new())),
            ExecOutcome::Trap => Err(anyhow!("Contract trapped")),
            ExecOutcome::OutOfGas => Err(anyhow!("Out of gas")),
        }
    }

    /// Handle syscall by pre-computed ID (O(1) dispatch, no string matching)
    ///
    /// Parameters are read from registers A0..A5 (matching pallet-revive's
    /// `read_input_regs` convention). Return values are written to A0 via
    /// the `ReturnValue` variant.
    #[inline]
    fn handle_syscall_by_id(
        &mut self,
        instance: &mut polkavm::RawInstance,
        syscall_id: SyscallId,
        calldata: &[u8],
    ) -> SyscallResult {
        let a0 = instance.reg(Reg::A0);
        let a1 = instance.reg(Reg::A1);
        let a2 = instance.reg(Reg::A2);

        match syscall_id {
            // ---- Call data functions ----
            SyscallId::CallDataSize => SyscallResult::ReturnValue(calldata.len() as u64),

            SyscallId::CallDataLoad => {
                let out_ptr = a0 as u32;
                let offset = a1 as u32;
                let mut data = [0u8; 32];
                let start = offset as usize;
                if start < calldata.len() {
                    let end = start.saturating_add(32).min(calldata.len());
                    data[..end - start].copy_from_slice(&calldata[start..end]);
                    data.reverse(); // Critical: BE→LE conversion (matches pallet-revive)
                }
                let _ = instance.write_memory(out_ptr, &data);
                SyscallResult::Continue
            }

            SyscallId::CallDataCopy => {
                let out_ptr = a0 as u32;
                let out_len = a1 as u32;
                let offset = a2 as u32;
                let start = offset as usize;
                if start >= calldata.len() {
                    let _ = instance.zero_memory(out_ptr, out_len);
                } else {
                    let end = start.saturating_add(out_len as usize).min(calldata.len());
                    let _ = instance.write_memory(out_ptr, &calldata[start..end]);
                    let written = (end - start) as u32;
                    if written < out_len {
                        let _ = instance.zero_memory(out_ptr + written, out_len - written);
                    }
                }
                SyscallResult::Continue
            }

            // ---- Control flow ----
            SyscallId::SealReturn => {
                let flags = a0 as u32;
                let data_ptr = a1 as u32;
                let data_len = a2 as u32;
                let mut buf = vec![0u8; data_len as usize];
                let _ = instance.read_memory_into(data_ptr, buf.as_mut_slice());
                SyscallResult::Terminate { flags, data: buf }
            }

            SyscallId::ConsumeAllGas => SyscallResult::Terminate {
                flags: 1, // REVERT
                data: Vec::new(),
            },

            // ---- Context info ----
            SyscallId::ValueTransferred => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0u8; 32]);
                SyscallResult::Continue
            }

            // ---- Immutable data ----
            SyscallId::SetImmutableData => SyscallResult::Continue,

            SyscallId::GetImmutableData => {
                let out_len_ptr = a1 as u32;
                let _ = instance.write_memory(out_len_ptr, &0u32.to_le_bytes());
                SyscallResult::Continue
            }

            // ---- Storage functions ----
            SyscallId::GetStorageOrZero => {
                let key_ptr = a1 as u32;
                let out_ptr = a2 as u32;
                let mut key = [0u8; 32];
                let _ = instance.read_memory_into(key_ptr, &mut key);
                let value = self.storage.get(&key).copied().unwrap_or([0u8; 32]);
                let _ = instance.write_memory(out_ptr, &value);
                SyscallResult::Continue
            }

            SyscallId::SetStorageOrClear => {
                let key_ptr = a1 as u32;
                let value_ptr = a2 as u32;
                let mut key = [0u8; 32];
                let _ = instance.read_memory_into(key_ptr, &mut key);
                let mut value = [0u8; 32];
                let _ = instance.read_memory_into(value_ptr, &mut value);

                let old_len = if self.storage.contains_key(&key) {
                    32u32
                } else {
                    u32::MAX // SENTINEL = key didn't exist
                };

                if value.iter().all(|&b| b == 0) {
                    self.storage.remove(&key);
                } else {
                    self.storage.insert(key, value);
                }
                SyscallResult::ReturnValue(old_len as u64)
            }

            // ---- Hashing ----
            SyscallId::HashKeccak256 => {
                let input_ptr = a0 as u32;
                let input_len = a1 as u32;
                let output_ptr = a2 as u32;
                let mut input = vec![0u8; input_len as usize];
                let _ = instance.read_memory_into(input_ptr, input.as_mut_slice());
                let hash = keccak256(&input);
                let _ = instance.write_memory(output_ptr, hash.as_slice());
                SyscallResult::Continue
            }

            // ---- Misc stubs ----
            SyscallId::Address => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0x42u8; 20]);
                SyscallResult::Continue
            }

            SyscallId::Caller => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0x01u8; 20]);
                SyscallResult::Continue
            }

            SyscallId::Balance => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0u8; 32]);
                SyscallResult::Continue
            }

            SyscallId::BlockNumber => {
                let out_ptr = a0 as u32;
                let mut buf = [0u8; 32];
                buf[0] = 1; // block 1 in LE
                let _ = instance.write_memory(out_ptr, &buf);
                SyscallResult::Continue
            }

            SyscallId::ReturnDataSize => SyscallResult::ReturnValue(0),

            SyscallId::RefTimeLeft => {
                let gas = instance.gas();
                SyscallResult::ReturnValue(gas as u64)
            }

            SyscallId::Unknown => SyscallResult::Continue,
        }
    }

    /// Handle a single syscall (Ecalli interrupt) - legacy string-based dispatch.
    ///
    /// Parameters are read from registers A0..A5 (matching pallet-revive's
    /// `read_input_regs` convention). Return values are written to A0 via
    /// the `ReturnValue` variant.
    #[allow(dead_code)]
    fn handle_syscall(
        &mut self,
        instance: &mut polkavm::RawInstance,
        symbol: &[u8],
        calldata: &[u8],
    ) -> SyscallResult {
        let a0 = instance.reg(Reg::A0);
        let a1 = instance.reg(Reg::A1);
        let a2 = instance.reg(Reg::A2);

        match symbol {
            // ---- Call data functions ----

            // call_data_size() -> u64
            b"call_data_size" => SyscallResult::ReturnValue(calldata.len() as u64),

            // call_data_load(out_ptr: u32, offset: u32)
            // Reads 32 bytes from calldata at offset, reverses for BE→LE conversion
            b"call_data_load" => {
                let out_ptr = a0 as u32;
                let offset = a1 as u32;
                let mut data = [0u8; 32];
                let start = offset as usize;
                if start < calldata.len() {
                    let end = start.saturating_add(32).min(calldata.len());
                    data[..end - start].copy_from_slice(&calldata[start..end]);
                    data.reverse(); // Critical: BE→LE conversion (matches pallet-revive)
                }
                let _ = instance.write_memory(out_ptr, &data);
                SyscallResult::Continue
            }

            // call_data_copy(out_ptr: u32, out_len: u32, offset: u32)
            // Raw byte copy without endianness conversion
            b"call_data_copy" => {
                let out_ptr = a0 as u32;
                let out_len = a1 as u32;
                let offset = a2 as u32;
                let start = offset as usize;
                if start >= calldata.len() {
                    let _ = instance.zero_memory(out_ptr, out_len);
                } else {
                    let end = start.saturating_add(out_len as usize).min(calldata.len());
                    let _ = instance.write_memory(out_ptr, &calldata[start..end]);
                    let written = (end - start) as u32;
                    if written < out_len {
                        let _ = instance.zero_memory(out_ptr + written, out_len - written);
                    }
                }
                SyscallResult::Continue
            }

            // ---- Control flow ----

            // seal_return(flags: u32, data_ptr: u32, data_len: u32)
            // Terminates execution (matching pallet-revive's TrapReason::Return)
            b"seal_return" => {
                let flags = a0 as u32;
                let data_ptr = a1 as u32;
                let data_len = a2 as u32;
                let mut buf = vec![0u8; data_len as usize];
                let _ = instance.read_memory_into(data_ptr, buf.as_mut_slice());
                SyscallResult::Terminate { flags, data: buf }
            }

            // consume_all_gas() - revert with no data
            b"consume_all_gas" => SyscallResult::Terminate {
                flags: 1, // REVERT
                data: Vec::new(),
            },

            // ---- Context info ----

            // value_transferred(out_ptr: u32) - write 32 bytes LE zero
            b"value_transferred" => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0u8; 32]);
                SyscallResult::Continue
            }

            // ---- Immutable data ----

            // set_immutable_data(ptr: u32, len: u32) - no-op for benchmarking
            b"set_immutable_data" => SyscallResult::Continue,

            // get_immutable_data(out_ptr: u32, out_len_ptr: u32) - return empty
            b"get_immutable_data" => {
                let _out_ptr = a0 as u32;
                let out_len_ptr = a1 as u32;
                // Write 0 length
                let _ = instance.write_memory(out_len_ptr, &0u32.to_le_bytes());
                SyscallResult::Continue
            }

            // ---- Storage functions ----

            // get_storage_or_zero(flags: u32, key_ptr: u32, out_ptr: u32)
            b"get_storage_or_zero" => {
                let _flags = a0 as u32;
                let key_ptr = a1 as u32;
                let out_ptr = a2 as u32;
                let mut key = [0u8; 32];
                let _ = instance.read_memory_into(key_ptr, &mut key);
                let value = self.storage.get(&key).copied().unwrap_or([0u8; 32]);
                let _ = instance.write_memory(out_ptr, &value);
                SyscallResult::Continue
            }

            // set_storage_or_clear(flags: u32, key_ptr: u32, value_ptr: u32) -> u32
            b"set_storage_or_clear" => {
                let _flags = a0 as u32;
                let key_ptr = a1 as u32;
                let value_ptr = a2 as u32;
                let mut key = [0u8; 32];
                let _ = instance.read_memory_into(key_ptr, &mut key);
                let mut value = [0u8; 32];
                let _ = instance.read_memory_into(value_ptr, &mut value);

                let old_len = if self.storage.contains_key(&key) {
                    32u32
                } else {
                    u32::MAX // SENTINEL = key didn't exist
                };

                if value.iter().all(|&b| b == 0) {
                    self.storage.remove(&key);
                } else {
                    self.storage.insert(key, value);
                }
                SyscallResult::ReturnValue(old_len as u64)
            }

            // ---- Hashing ----

            // hash_keccak_256(input_ptr: u32, input_len: u32, output_ptr: u32)
            b"hash_keccak_256" => {
                let input_ptr = a0 as u32;
                let input_len = a1 as u32;
                let output_ptr = a2 as u32;
                let mut input = vec![0u8; input_len as usize];
                let _ = instance.read_memory_into(input_ptr, input.as_mut_slice());
                let hash = keccak256(&input);
                let _ = instance.write_memory(output_ptr, hash.as_slice());
                SyscallResult::Continue
            }

            // ---- Misc stubs for contracts that import but may not call ----

            b"address" => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0x42u8; 20]);
                SyscallResult::Continue
            }

            b"caller" => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0x01u8; 20]);
                SyscallResult::Continue
            }

            b"balance" => {
                let out_ptr = a0 as u32;
                let _ = instance.write_memory(out_ptr, &[0u8; 32]);
                SyscallResult::Continue
            }

            b"block_number" => {
                let out_ptr = a0 as u32;
                let mut buf = [0u8; 32];
                buf[0] = 1; // block 1 in LE
                let _ = instance.write_memory(out_ptr, &buf);
                SyscallResult::Continue
            }

            b"return_data_size" => SyscallResult::ReturnValue(0),

            b"ref_time_left" => {
                let gas = instance.gas();
                SyscallResult::ReturnValue(gas as u64)
            }

            _ => {
                #[cfg(test)]
                eprintln!(
                    "WARNING: Unhandled syscall: {:?}",
                    std::str::from_utf8(symbol)
                );
                SyscallResult::Continue
            }
        }
    }
}

impl Default for PolkaVmExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create PolkaVM executor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{self, WarpContract};

    #[test]
    fn test_executor_creation() {
        let result = PolkaVmExecutor::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_module_info() {
        let executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Arithmetic).unwrap();
        let module = executor.load_module(&bytecode).unwrap();
        executor.print_module_info(&module);
    }

    #[test]
    fn test_call_arithmetic_compute() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Arithmetic).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        let calldata = contracts::arithmetic::encode_compute(
            alloy_primitives::U256::from(1234578u64),
            alloy_primitives::U256::from(67890u64),
        );

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());

        let (gas_used, output) = result.unwrap();
        eprintln!("Gas used: {}, Output: {:?}", gas_used, output);
        assert!(!output.is_empty(), "Output should not be empty");
    }

    #[test]
    fn test_call_arithmetic_compute_many() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Arithmetic).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        let calldata =
            contracts::arithmetic::encode_compute_many(alloy_primitives::U256::from(10000u64));

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());
    }

    #[test]
    fn test_call_loop_simple() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Loop).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        let calldata =
            contracts::loop_contract::encode_simple_loop(alloy_primitives::U256::from(100u64));

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());
    }

    #[test]
    fn test_call_storage_write() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Storage).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        let calldata =
            contracts::storage::encode_write_sequential(alloy_primitives::U256::from(5u64));

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());
    }

    #[test]
    fn test_call_keccak_hash_many() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Keccak256).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        let calldata =
            contracts::keccak256::encode_hash_many(alloy_primitives::U256::from(10u64));

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());

        let (gas_used, output) = result.unwrap();
        eprintln!("Gas used: {}, Output: {:?}", gas_used, output);
        assert_eq!(output.len(), 32, "Keccak256 output should be 32 bytes");
    }

    #[test]
    fn test_call_keccak_hash_variable_size() {
        let mut executor = PolkaVmExecutor::new().unwrap();
        let bytecode = contracts::load_polkavm_bytecode(WarpContract::Keccak256).unwrap();
        let module = executor.load_module(&bytecode).unwrap();

        // Test with 64 bytes of data
        let calldata =
            contracts::keccak256::encode_hash_variable_size(alloy_primitives::U256::from(64u64));

        let result = executor.call_with_data(&module, &calldata);
        eprintln!("Result: {:?}", result);
        assert!(result.is_ok(), "Call failed: {:?}", result.err());

        let (gas_used, output) = result.unwrap();
        eprintln!("Gas used: {}, Output: {:?}", gas_used, output);
        assert_eq!(output.len(), 32, "Keccak256 output should be 32 bytes");
    }
}
