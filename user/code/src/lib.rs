#![no_std]
#![feature(const_refs_to_static)]
use core::result::Result::*; 
use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::addr_of_mut;
use core::ptr::write_volatile;


pub extern crate alloc;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}


const PAGE_SIZE : usize = 16 * 1024;
const USER_IMG_BASE : usize = 0xffff_ffff_c000_0000;

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
        &__text_beg as *const _ as u64, &__text_end as *const _ as u64
    );
    println!(
        "bss  beg: {:016x}, end: {:016x}",
        &__bss_beg as *const _ as u64, &__bss_end as *const _ as u64
    );
}

extern "Rust" {
    fn main();
}

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    zeros_bss();
    main();
    kernel_api::syscall::exit();
}


struct InnerAlloc(UnsafeCell<(usize, usize)>);

use core::marker::{Send, Sync};
unsafe impl Send for InnerAlloc {}

unsafe impl Sync for InnerAlloc {}

pub struct GlobalAllocator(InnerAlloc);

extern "C" {
    static __text_end: u8;
}
impl GlobalAllocator {
    const fn new() -> Self {
        let mut binary_end = unsafe { (&__text_end as *const u8) as usize };
        unsafe {
            return GlobalAllocator(InnerAlloc(UnsafeCell::new((__text_end as usize, __text_end as usize +PAGE_SIZE)))) 
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let (beg, end) = &mut *self.0.0.get();

            if *beg & (layout.align() - 1) != 0 {
                *beg = *beg & (!(layout.align() - 1)) + layout.align();
            }

            let location = *beg as *mut u8;
            *beg += layout.size();

            location
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
#[global_allocator]
pub static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();


pub unsafe fn get_data() -> usize {
    let ga = &ALLOCATOR;
    let (beg, end) = &mut *ga.0.0.get();
    return *beg;
}

use core::fmt;
use core::fmt::Write;
use alloc::string::String;
use kernel_api::syscall::*;
struct Console;


impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);

        Ok(())
    }
}


pub fn vprint(s: String) {
    let mut c = Console;
    c.write_str(s.as_str()).unwrap();
}
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => (crate::vprint(crate::alloc::format!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ({
        crate::vprint(crate::alloc::format!("{}\n", crate::alloc::format!($($arg)*)));
    })
}
