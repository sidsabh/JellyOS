#![no_std]
#![no_main]

extern crate alloc;
use user::*;
use kernel_api::*;

#[no_mangle]
fn main(argc: usize, argv_ptr: *const *const u8) {
    // Fork chain of 10 processes recursively:
    fork_chain(10);
}

fn fork_chain(n: usize) {
    if n == 0 {
        syscall::exit();
    }

    let pid = syscall::fork().unwrap();
    if pid == 0 {
        // Child process
        fork_chain(n - 1);
        syscall::exit();
    } else if pid > 0 {
        // Parent process
        let _ = syscall::wait(pid);
    } else {
        // Fork failed
        panic!("Fork failed");
    }
}