use core::fmt;
use core::fmt::Write;
use alloc::string::String;
use kernel_api::syscall::*;
use core::result::Result::*; 
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
    ($($arg:tt)*) => (crate::console::vprint(crate::alloc::format!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ({
        crate::console::vprint(crate::alloc::format!("{}\n", crate::alloc::format!($($arg)*)));
    })
}
