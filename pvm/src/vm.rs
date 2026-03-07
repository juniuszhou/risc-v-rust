//! PolkaVM virtual machine execution logic.
//!
//! Initializes the engine, loads bytecode, and executes guest programs.

use polkavm::{Config, Engine, Linker, Module, ModuleConfig, ProgramBlob, Reg, RegValue};

/// Default PolkaVM configuration using the interpreter backend.
/// The interpreter works on all platforms without requiring a Linux sandbox.
fn default_config() -> Config {
    let mut config = Config::default();
    // Use interpreter backend for maximum compatibility (no Linux sandbox required)
    config.set_backend(Some(polkavm::BackendKind::Interpreter));
    config
}

/// Initializes the PolkaVM engine with default configuration.
///
/// # Errors
/// Returns an error if the engine cannot be created (e.g., invalid config).
pub fn init_engine() -> Result<Engine, polkavm::Error> {
    let config = default_config();
    Engine::new(&config)
}

/// Loads PolkaVM bytecode from raw bytes into a Module.
///
/// # Arguments
/// * `engine` - The initialized PolkaVM engine
/// * `bytecode` - Raw PolkaVM program blob bytes (e.g., from a .polkavm file)
///
/// # Errors
/// Returns an error if the bytecode is invalid or cannot be parsed.
pub fn load_module(engine: &Engine, bytecode: &[u8]) -> Result<Module, polkavm::Error> {
    let bytes = polkavm::ArcBytes::from(bytecode);
    let config = ModuleConfig::default();
    Module::new(engine, &config, bytes)
}

/// Runs a loaded module by calling an exported function.
///
/// # Arguments
/// * `module` - The loaded PolkaVM module
/// * `export_name` - Name of the exported function to call (e.g., "main")
/// * `args` - Arguments to pass (e.g., `(42u32,)` for one u32 argument)
///
/// # Returns
/// The return value from register A0 (first return register in RISC-V ABI).
///
/// # Errors
/// Returns an error if the export is not found or execution fails.
pub fn run_export(
    module: &Module,
    export_name: &str,
    args: (),
) -> Result<RegValue, polkavm::CallError<std::convert::Infallible>> {
    let linker = Linker::<(), std::convert::Infallible>::new();
    let instance_pre = linker
        .instantiate_pre(module)
        .map_err(polkavm::CallError::Error)?;

    let mut instance = instance_pre
        .instantiate()
        .map_err(polkavm::CallError::Error)?;

    instance.set_gas(i64::MAX);
    instance.call_typed(&mut (), export_name, args)?;

    Ok(instance.reg(Reg::A0))
}

/// High-level runner that initializes the VM and executes bytecode.
pub struct VmRunner {
    engine: Engine,
}

impl VmRunner {
    /// Creates a new VM runner with an initialized engine.
    pub fn new() -> Result<Self, polkavm::Error> {
        let engine = init_engine()?;
        Ok(Self { engine })
    }

    /// Loads bytecode and returns the module.
    pub fn load(&self, bytecode: &[u8]) -> Result<Module, polkavm::Error> {
        load_module(&self.engine, bytecode)
    }

    /// Loads bytecode from a file path.
    pub fn load_from_path(&self, path: &std::path::Path) -> Result<Module, polkavm::Error> {
        let bytecode = std::fs::read(path)
            .map_err(|e| polkavm::Error::from(format!("failed to read file: {e}")))?;
        self.load(&bytecode)
    }

    /// Loads bytecode and runs the given export.
    pub fn load_and_run(
        &self,
        bytecode: &[u8],
        export_name: &str,
    ) -> Result<RegValue, polkavm::CallError<std::convert::Infallible>> {
        let module = self.load(bytecode).map_err(polkavm::CallError::Error)?;
        run_export(&module, export_name, ())
    }
}

/// Convenience function: init engine, load bytecode, run "main" with no args.
pub fn load_and_run(
    bytecode: &[u8],
) -> Result<RegValue, polkavm::CallError<std::convert::Infallible>> {
    let runner = VmRunner::new().map_err(polkavm::CallError::Error)?;
    runner.load_and_run(bytecode, "main")
}

/// Executes a simple program from a ProgramBlob (used by tests).
#[doc(hidden)]
pub fn execute_blob(
    blob: ProgramBlob,
) -> Result<u32, polkavm::CallError<std::convert::Infallible>> {
    let engine = init_engine().map_err(polkavm::CallError::Error)?;
    let module = Module::from_blob(&engine, &ModuleConfig::default(), blob)
        .map_err(polkavm::CallError::Error)?;
    let result = run_export(&module, "main", ())?;
    Ok(result as u32)
}
