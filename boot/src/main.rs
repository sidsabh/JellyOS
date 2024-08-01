#![feature(asm)]
#![feature(global_asm)]

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]


#[cfg(not(test))]

mod init;

use pi::uart::MiniUart;
use shim::io::Write;
use xmodem::Xmodem;
use core::time::Duration;
use pi;
use core::arch::asm;

/// Start address of the binary to load and of the bootloader.
const BINARY_START_ADDR: usize = 0x80000;
const BOOTLOADER_START_ADDR: usize = 0x4000000;

/// Pointer to where the loaded binary expects to be laoded.
const BINARY_START: *mut u8 = BINARY_START_ADDR as *mut u8;

/// Free space between the bootloader and the loaded binary's start address.
const MAX_BINARY_SIZE: usize = BOOTLOADER_START_ADDR - BINARY_START_ADDR;

/// Branches to the address `addr` unconditionally.
unsafe fn jump_to(addr: *mut u8) -> ! {
    asm!(
        "br {dest}",
        dest = in(reg) addr as usize,
        options(noreturn)
    )
    // asm!("br $0" : : "r"(addr as usize));
    // loop {
    //     asm!("wfe" :::: "volatile")
    // }
}

use core::slice::from_raw_parts_mut;
fn kmain() -> ! {
    // FIXME: Implement the bootloader.
    loop {
        let into: &mut [u8] = unsafe {from_raw_parts_mut(BINARY_START, MAX_BINARY_SIZE)};
        let mut from = MiniUart::new();
        from.set_read_timeout(Duration::from_millis(750));
        match Xmodem::receive(from, into) {
            Ok(_) => {
                break;
            }, 
            Err(_) => {
                // MiniUart::new().write("Failed to load binary\n".as_bytes()).expect("Failed to write fail message");
            }
        }
    }
    unsafe {
        jump_to(BINARY_START);
    }
}
