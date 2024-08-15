#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
// features
#![allow(internal_features)]
#![feature(ptr_internals)]
#![feature(raw_vec_internals)]
#![feature(prelude_2024)]
#![feature(alloc_error_handler)]
#![feature(decl_macro)]
#![feature(auto_traits)]
#![feature(negative_impls)]
#![feature(const_mut_refs)]
#![feature(const_option)]
#![feature(let_chains)]
#![feature(asm_const)]

// #[cfg(not(test))] // commenting for rust-analyzer
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;
pub mod param;
pub mod process;
pub mod traps;
pub mod vm;



use shell::shell;

use allocator::Allocator;
use fs::FileSystem;
use process::GlobalScheduler;
use traps::irq::Irq;
use vm::VMManager;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
pub static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
pub static VMM: VMManager = VMManager::uninitialized();
pub static IRQ: Irq = Irq::uninitialized();

use crate::console::kprintln;
use pi::timer::spin_sleep;
use core::time::Duration;

use aarch64::current_el;

fn kmain() -> ! {
    spin_sleep(Duration::from_millis(500)); // necessary delay after transmit before tty

    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }

    unsafe {
        kprintln!("{}", current_el());
    }

    shell(">");
}

