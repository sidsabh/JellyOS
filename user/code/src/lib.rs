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

    println!("heap beg: {:016x}, end: {:016x}", start, end);

    // initialize logger
    init_logger();
    // if syscall::getpid() != 0 {
    //     return;
    // }
    
    debug!("text beg: {:016x}, end: {:016x}",
        addr_of!(__text_beg) as *const _ as u64, addr_of!(__text_end) as *const _ as u64
    );
    debug!(
        "bss  beg: {:016x}, end: {:016x}",
        addr_of!(__bss_beg) as *const _ as u64, addr_of!(__bss_end) as *const _ as u64
    );
    // trace!("heap beg: {:016x}, end: {:016x}", start, end);
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
        stp x0, x1, [SP, #-16]!

        bl setup_memory   // Call setup_memory()

        ldp x0, x1, [SP], #16

        bl main           // Call main(argc, argv)

        bl call_exit       // Exit when main returns
        ",
    );
}

#[no_mangle]
pub extern "C" fn debug_print_regs(argc: usize, argv: *const *const u8) {
    println!("[DEBUG] _start before main: argc = {}, argv_ptr = {:#x}", argc, argv as usize);
}


use alloc::vec::Vec;
use alloc::string::String;
use core::ptr;
use core::str::from_utf8;



pub fn get_args(argc: usize, argv_ptr: *const *const u8) -> Vec<String> {
    println!("[DEBUG] _start before main: argc = {}, argv_ptr = {:#x}", argc, argv_ptr as usize);
    let mut args: Vec<String> = Vec::new();
    for i in 0..argc {
        let arg_ptr = unsafe { *argv_ptr.add(i) };
        if arg_ptr.is_null() {
            break;
        }
        let mut len = 0;
        unsafe {
            while ptr::read(arg_ptr.add(len)) != 0 {
                len += 1;
            }
        }
        let arg_slice = unsafe { core::slice::from_raw_parts(arg_ptr, len) };
        let arg_str = from_utf8(arg_slice).unwrap_or("[Invalid UTF-8]");
        args.push(String::from(arg_str));
    }
    args
}
