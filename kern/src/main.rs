#![no_std]
#![feature(prelude_2024)]
#![feature(alloc_error_handler)]
// #![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(auto_traits)]
// #![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(negative_impls)]

#[cfg(not(test))]
mod init;

pub mod console;
pub mod mutex;
pub mod shell;

use shell::shell;
use shim::io::Write;

// FIXME: You need to add dependencies here to
// test your drivers (Phase 2). Add them as needed.

fn kmain() -> ! {
    shell("#")
}

use pi::uart::MiniUart;

fn echo_uart() -> ! {
    let mut uart = MiniUart::new();
    loop {
        let byte = uart.read_byte();
        uart.write_byte(b'\n');
        uart.write(&[byte]).expect("error writing char");
        uart.flush().expect("error flushing");
    }
}

use core::time::Duration;
use pi::gpio::Gpio;
use pi::timer::spin_sleep;
fn loading_spinner() -> ! {
    let top_left = Gpio::new(5).into_output();
    let top_right = Gpio::new(6).into_output();
    let left = Gpio::new(16).into_output();
    let right = Gpio::new(13).into_output();
    let bottom_left = Gpio::new(19).into_output();
    let bottom_right = Gpio::new(26).into_output();

    let delay = Duration::from_millis(100);

    let mut pins = [top_left, top_right, right, bottom_right, bottom_left, left];
    let mut i: usize = 0;
    loop {
        pins[i].set();
        pins[((i as i32) - 1).rem_euclid(pins.len() as i32) as usize].clear();
        spin_sleep(delay);
        i = (i + 1) % pins.len();
    }
}
