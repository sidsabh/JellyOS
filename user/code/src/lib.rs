#![no_std]
#![feature(const_refs_to_static)]
use core::ptr::addr_of;
use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::addr_of_mut;
use core::ptr::write_volatile;

pub mod allocator;
pub mod console;

pub extern crate alloc;

use allocator::{ALLOCATOR, memory_map};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

unsafe fn zeros_bss() {
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
    println!("text beg: {:016x}, end: {:016x}",
        addr_of!(__text_beg) as *const _ as u64, addr_of!(__text_end) as *const _ as u64
    );
    println!(
        "bss  beg: {:016x}, end: {:016x}",
        addr_of!(__bss_beg) as *const _ as u64, addr_of!(__bss_end) as *const _ as u64
    );
}

extern "Rust" {
    fn main();
}

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    zeros_bss();
    let (start, end) = memory_map();
    println!("heap beg: {:016x}, end: {:016x}", start, end);
    ALLOCATOR.initialize(start, end);
    main();
    kernel_api::syscall::exit();
}

