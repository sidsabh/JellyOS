#![no_std]
#![no_main]

use user::*;
use core::ptr;
use core::str::from_utf8;

#[no_mangle]
fn main(argc: usize, argv_ptr: *const *const u8) {
    println!("argc = {}", argc);

    // Iterate over argv and print each argument.
    for i in 0..argc {
        let arg_ptr = unsafe { *argv_ptr.add(i) };
        if arg_ptr.is_null() {
            break;
        }

        // Read until null terminator
        let mut len = 0;
        unsafe {
            while ptr::read(arg_ptr.add(len)) != 0 {
                len += 1;
            }
        }

        // Convert to &str
        let arg_str = unsafe {
            from_utf8(core::slice::from_raw_parts(arg_ptr, len)).unwrap_or("[Invalid UTF-8]")
        };

        println!("argv[{}] = '{}'", i, arg_str);
    }
}
