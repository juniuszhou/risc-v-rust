//! Integration tests for the PolkaVM host runner.

use pvm::{init_engine, load_and_run, load_module, run_export, VmRunner};
use polkavm::ProgramBlob;
use polkavm_common::program::{asm, InstructionSetKind};
use polkavm_common::program::Reg::*;
use polkavm_common::writer::ProgramBlobBuilder;

/// Creates a minimal PolkaVM program that adds 666 to A0 and returns.
fn make_add_program() -> Vec<u8> {
    let mut builder = ProgramBlobBuilder::new(InstructionSetKind::Latest32);
    builder.add_export_by_basic_block(0, b"main");
    builder.set_code(
        &[
            asm::fallthrough(),
            asm::add_imm_32(A0, A0, 666),
            asm::ret(),
        ],
        &[],
    );
    builder.into_vec().unwrap()
}

#[test]
fn test_init_engine() {
    let engine = init_engine().expect("engine init");
    assert!(engine.backend().to_string().len() > 0);
}

#[test]
fn test_vm_runner_new() {
    let runner = VmRunner::new().expect("VmRunner::new");
    let _ = runner; // use it
}

#[test]
fn test_load_and_execute_bytecode() {
    let bytecode = make_add_program();
    let result = load_and_run(&bytecode).expect("load_and_run");
    // Program adds 666 to A0 (initially 0), returns 666
    assert_eq!(result, 666);
}

#[test]
fn test_run_export() {
    let bytecode = make_add_program();
    let engine = init_engine().expect("engine");
    let module = load_module(&engine, &bytecode).expect("load_module");
    let result = run_export(&module, "main", ()).expect("run_export");
    assert_eq!(result, 666);
}

#[test]
fn test_execute_blob() {
    let bytecode = make_add_program();
    let blob = ProgramBlob::parse(polkavm::ArcBytes::from(bytecode)).expect("parse blob");
    let result = pvm::execute_blob(blob).expect("execute_blob");
    assert_eq!(result, 666);
}

#[test]
fn test_load_from_file() {
    let bytecode = make_add_program();
    let temp_path = std::env::temp_dir().join("pvm_test_add.polkavm");
    std::fs::write(&temp_path, &bytecode).expect("write temp file");

    let runner = VmRunner::new().expect("runner");
    let module = runner.load_from_path(&temp_path).expect("load from path");
    let result = run_export(&module, "main", ()).expect("run");
    assert_eq!(result, 666);

    let _ = std::fs::remove_file(&temp_path); // best-effort cleanup
}
