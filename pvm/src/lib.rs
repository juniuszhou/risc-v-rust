//! PolkaVM host application to execute RISC-V bytecode.
//!
//! This crate provides utilities to:
//! 1. Initialize the PolkaVM engine
//! 2. Load compiled PolkaVM bytecode
//! 3. Execute the bytecode and call exported functions

mod vm;

pub use vm::{
    execute_blob, init_engine, load_and_run, load_module, run_export, VmRunner,
};
pub use polkavm::{Config, Engine, Module, ModuleConfig, ProgramBlob, ProgramCounter, Reg};
