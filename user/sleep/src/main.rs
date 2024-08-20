#![feature(asm)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

use core::arch::asm;
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!(
                "mov x0, {ms:x}",
                "svc 1",
                "mov {ems}, x0",
                "mov {error}, x7",
                ms = in(reg) ms,
                ems = out(reg) elapsed_ms,
                error = out(reg) error,
                out("x0") _,   // Clobbers x0
                out("x7") _,   // Clobbers x7
                options(nostack),
            );
        }
    }
}
