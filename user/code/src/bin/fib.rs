#![no_std]
#![no_main]
use user::*;

use kernel_api::syscall::*;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

#[no_mangle]
fn main() {
    let pid = getpid();
    let beg = time();
    println!("[{:02}] Started: {:?}", pid, beg);
    let rtn = fib(40);
    let end = time();

    println!("[{:02}] Ended: {:?}\n", pid, end);
    println!("[{:02}] Result: {} ({:?})", pid, rtn, end - beg);
}
