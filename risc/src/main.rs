#![no_std]
#![no_main]

use core::panic::PanicInfo;

// QEMU UART address for the virt machine
const UART_ADDR: usize = 0x1000_0000;

// QEMU test device for program exit
const SIFIVE_TEST_ADDR: usize = 0x100000;
const EXIT_SUCCESS: u32 = 0x5555;

// Write a single byte to the UART
fn uart_write(byte: u8) {
    unsafe {
        core::ptr::write_volatile(UART_ADDR as *mut u8, byte);
    }
}

// Write a string to the UART
fn print_str(s: &str) {
    for byte in s.bytes() {
        uart_write(byte);
    }
}

// Exit QEMU
fn exit_qemu(code: u32) -> ! {
    unsafe {
        core::ptr::write_volatile(SIFIVE_TEST_ADDR as *mut u32, code);
    }
    // In case the exit doesn't work
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_str("PANIC!");
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Print "Hello, world!" to the UART
    print_str("Hello, world!\n");

    // Exit QEMU (only works when QEMU is run with -device sifive_test)
    exit_qemu(EXIT_SUCCESS);
}
