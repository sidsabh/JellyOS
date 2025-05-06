#![no_std]
#![no_main]
use user::*;

#[no_mangle]
fn main(argc: usize, argv_ptr: *const *const u8) {
    for _ in 0..10 {
        let ms = 1000;
        let _error: u64;
        let _elapsed_ms: u64;

        unsafe {
            core::arch::asm!(
                "mov x0, {ms:x}",
                "svc 1",
                "mov {ems}, x0",
                "mov {error}, x7",
                ms = in(reg) ms,
                ems = out(reg) _elapsed_ms,
                error = out(reg) _error,
                out("x0") _,   // Clobbers x0
                out("x7") _,   // Clobbers x7
                options(nostack),
            );
        }
    }
    info!("sleep done");
}
