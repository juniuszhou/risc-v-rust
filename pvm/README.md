# PolkaVM Host Runner

Execute RISC-V bytecode using the [PolkaVM](https://github.com/paritytech/polkavm) virtual machine.

## Overview

1. **Initialize** the PolkaVM engine
2. **Load** compiled bytecode (`.polkavm` format)
3. **Execute** exported functions

## Building

```bash
cargo build
```

The pvm crate builds for the host (x86_64) by default via `.cargo/config.toml`, since it runs the VM rather than being compiled for RISC-V.

## Usage

```bash
cargo run -- <path-to-bytecode.polkavm>
```

Example with the fibonacci contract (requires building it first):

```bash
cd ../fibonacci-rust && ./build.sh
cd ../pvm && cargo run -- ../fibonacci-rust/fibonacci.polkavm
```

Note: The fibonacci contract uses host imports (pallet-revive-uapi) and will fail at instantiation without a full runtime. For programs with no imports, execution works out of the box.

## Library API

```rust
use pvm::{VmRunner, load_and_run};

// High-level: load and run in one call
let bytecode = std::fs::read("program.polkavm")?;
let result = load_and_run(&bytecode)?;

// Or step by step
let runner = VmRunner::new()?;
let module = runner.load(&bytecode)?;
let result = pvm::run_export(&module, "main", ())?;
```

## Testing

```bash
cargo test
```

Tests use `polkavm-common` to build minimal bytecode in-memory (a program that adds 666 to A0 and returns).
