#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
// features
#![feature(prelude_2024)]
#![feature(alloc_error_handler)]
#![feature(decl_macro)]
#![feature(auto_traits)]
#![feature(negative_impls)]
#![feature(const_mut_refs)]
#![feature(const_option)]
#![feature(let_chains)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

use shell::shell;

use allocator::Allocator;
use fs::FileSystem;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();

use crate::console::kprintln;
use pi::timer::spin_sleep;
use core::time::Duration;

fn kmain() -> ! {
    spin_sleep(Duration::from_millis(500)); // necessary delay after transmit before tty

    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }

    shell(">");
}

