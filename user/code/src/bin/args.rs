#![no_std]
#![no_main]

extern crate alloc;
use user::*;

#[no_mangle]
fn main(argc: usize, argv_ptr: *const *const u8) {
    let args = get_args(argc, argv_ptr);
    println!("args: {:?}", args);
}
