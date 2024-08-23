#![feature(asm)]
#![no_std]
#![no_main]
mod cr0;

extern crate alloc;

use kernel_api::*;
use kernel_api::syscall::*;
use crate::alloc::string::*;
use crate::alloc::format;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    let pid = getpid();
    let beg = time();
    // print!("{}", format!("[{:02}] Started: {:?}\n", pid, beg));
    let rtn = fib(40);
    let end = time();
    // print!("{}", format!("[{:02}] Ended: {:?}\n", pid, end));
    print!("{}", format!("[{:02}] Result: {} ({:?})\n", pid, rtn, end - beg));
}
