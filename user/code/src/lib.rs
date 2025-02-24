#![no_std]
#![feature(naked_functions)]

use core::ptr::addr_of;
use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::addr_of_mut;
use core::ptr::write_volatile;

pub mod allocator;
pub mod console;
pub mod logger;
pub extern crate alloc;

use allocator::{ALLOCATOR, memory_map};
use kernel_api::syscall;
use log::log;
use logger::init_logger;

pub use log::{info, warn, trace, debug, error};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    error!("User program crashed!");
    if let Some(location) = _info.location() {
        error!(
            "FILE: {}\nLINE: {}\nCOL: {}\n\n{}",
            location.file(),
            location.line(),
            location.column(),
            _info.message()
        );
    }
    syscall::exit();
}

#[no_mangle]
unsafe fn setup_memory() {
    // zero bss
    extern "C" {
        static mut __text_beg: u64;
        static mut __text_end: u64;
        static mut __bss_beg: u64;
        static mut __bss_end: u64;
    }

    let mut iter: *mut u64 = addr_of_mut!(__bss_beg);
    let end: *mut u64 = addr_of_mut!(__bss_end);

    while iter < end {
        write_volatile(iter, zeroed());
        iter = iter.add(1);
    }

    // initialize heap 
    let (start, end) = memory_map();
    ALLOCATOR.initialize(start, end);

    // initialize logger
    init_logger();
    if syscall::getpid() != 0 {
        return;
    }
    
    debug!("text beg: {:016x}, end: {:016x}",
        addr_of!(__text_beg) as *const _ as u64, addr_of!(__text_end) as *const _ as u64
    );
    debug!(
        "bss  beg: {:016x}, end: {:016x}",
        addr_of!(__bss_beg) as *const _ as u64, addr_of!(__bss_end) as *const _ as u64
    );
    trace!("heap beg: {:016x}, end: {:016x}", start, end);
}

extern "Rust" {
    fn main();
}

#[no_mangle]
fn call_exit() -> ! {
    syscall::exit();
}


#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        mov x10, x0       // Save argc
        mov x11, x1       // Save argv

        bl setup_memory     // Call setup_memory()

        // Restore registers before calling main()
        mov x0, x10
        mov x1, x11
        bl main


        bl call_exit       // Exit when main returns
        ",
    );
}
