#![no_std]
#![no_main]

// extra features
#![allow(internal_features)]
#![feature(ptr_internals)]
#![feature(raw_vec_internals)]
#![feature(alloc_error_handler)]
#![feature(decl_macro)]
#![feature(auto_traits)]
#![feature(negative_impls)]
#![feature(let_chains)]
#![feature(iter_chain)]
#![feature(if_let_guard)]
#![feature(array_chunks)]

// external crates
extern crate alloc;
#[macro_use]
extern crate log;
extern crate heap;

// import files
mod init;
mod allocator;
mod console;
mod fs;
mod logger;
mod mutex;
mod net;
mod param;
mod percore;
mod process;
mod shell;
mod traps;
mod vm;

use allocator::Allocator;
use fs::FileSystem;
use net::uspi::Usb;
use net::GlobalEthernetDriver;
use process::GlobalScheduler;
use shell::shell;
use traps::irq::{Fiq, GlobalIrq, LocalIrq};
use vm::VMManager;

#[global_allocator]
static ALLOCATOR: Allocator = Allocator::uninitialized();
static FILESYSTEM: FileSystem = FileSystem::uninitialized();
static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
static VMM: VMManager = VMManager::uninitialized();
static USB: Usb = Usb::uninitialized();
static GLOBAL_IRQ: GlobalIrq = GlobalIrq::new();
static IRQ: LocalIrq = LocalIrq::new();
static FIQ: Fiq = Fiq::new();
static ETHERNET: GlobalEthernetDriver = GlobalEthernetDriver::uninitialized();

use crate::console::kprintln;
use pi::timer::spin_sleep;
use core::time::Duration;

extern "C" {
    static __text_beg: u64;
    static __text_end: u64;
    static __bss_beg: u64;
    static __bss_end: u64;
}

unsafe fn log_layout() {
    crate::logger::init_logger();

    info!(
        "text beg: {:016x}, end: {:016x}",
        &__text_beg as *const _ as u64, &__text_end as *const _ as u64
    );
    info!(
        "bss  beg: {:016x}, end: {:016x}",
        &__bss_beg as *const _ as u64, &__bss_end as *const _ as u64
    );
}

/// bootstrapping core
unsafe fn kmain() -> ! {

    spin_sleep(Duration::from_millis(500)); // delay after transmit for serial console
    log_layout();
    ALLOCATOR.initialize();
    FILESYSTEM.initialize();
    VMM.initialize();
    SCHEDULER.initialize();
    // shell("$");
    init::initialize_app_cores();
    per_core_main()
}

unsafe fn per_core_main() -> ! {
    VMM.wait();
    SCHEDULER.start()
}
