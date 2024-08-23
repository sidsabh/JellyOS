use core::fmt;
use core::fmt::Write;
use core::time::Duration;

use crate::*;

macro_rules! err_or {
    ($ecode:expr, $rtn:expr) => {{
        let e = OsError::from($ecode);
        if let OsError::Ok = e {
            Ok($rtn)
        } else {
            Err(e)
        }
    }};
}
use core::arch::asm;

pub fn sleep(span: Duration) -> OsResult<Duration> {
    if span.as_millis() > core::u64::MAX as u128 {
        panic!("too big!");
    }

    let ms = span.as_millis() as u64;
    let mut ecode: u64;
    let mut elapsed_ms: u64;

    unsafe {
        asm!(
            "mov x0, {ms}",
            "svc {nr_sleep}",
            "mov {elapsed_ms}, x0",
            "mov {ecode}, x7",
            ms = in(reg) ms,
            nr_sleep = const NR_SLEEP,
            elapsed_ms = out(reg) elapsed_ms,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, Duration::from_millis(elapsed_ms))
}

pub fn time() -> Duration {
    let mut ecode: u64;
    let mut current_time: u64;
    let mut frac_time: u64;

    unsafe {
        asm!(
            "svc {nr_time}",
            "mov {current_time}, x0",
            "mov {frac_time}, x1",
            "mov {ecode}, x7",
            nr_time = const NR_TIME,
            current_time = out(reg) current_time,
            frac_time = out(reg) frac_time,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x1") _,   // Clobbers x0
            out("x7") _,   // Clobbers x0
            options(nostack),
        );
    }

    let _ = OsError::from(ecode);
    Duration::from_secs(current_time) + Duration::from_nanos(frac_time)
}

pub fn exit() -> ! {
    unsafe {
        asm!(
            "svc {nr_exit}",
            nr_exit = const NR_EXIT,
            options(nostack),
        );
    }
    loop {}
}

pub fn write(b: u8) {
    let mut ecode: u64;

    unsafe {
        asm!(
            "mov w0, {b:w}",
            "svc {nr_write}",
            "mov {ecode}, x7",
            b = in(reg) b,
            nr_write = const NR_WRITE,
            ecode = out(reg) ecode,
            out("x7") _,   // Clobbers x0
            options(nostack),
        );
    }

    let _ = OsError::from(ecode);
}

pub fn getpid() -> u64 {
    let mut ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!(
            "svc {nr_time}",
            "mov {pid}, x0",
            "mov {ecode}, x7",
            nr_time = const NR_TIME,
            pid = out(reg) pid,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x0
            options(nostack),
        );
    }

    let _ = OsError::from(ecode);

    pid
}

pub fn sock_create() -> SocketDescriptor {
    // Lab 5 2.D
    unimplemented!("sock_create")
}

pub fn sock_status(descriptor: SocketDescriptor) -> OsResult<SocketStatus> {
    // Lab 5 2.D
    unimplemented!("sock_status")
}

pub fn sock_connect(descriptor: SocketDescriptor, addr: IpAddr) -> OsResult<()> {
    // Lab 5 2.D
    unimplemented!("sock_connect")
}

pub fn sock_listen(descriptor: SocketDescriptor, local_port: u16) -> OsResult<()> {
    // Lab 5 2.D
    unimplemented!("sock_listen")
}

pub fn sock_send(descriptor: SocketDescriptor, buf: &[u8]) -> OsResult<usize> {
    // Lab 5 2.D
    unimplemented!("sock_send")
}

pub fn sock_recv(descriptor: SocketDescriptor, buf: &mut [u8]) -> OsResult<usize> {
    // Lab 5 2.D
    unimplemented!("sock_recv")
}

struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            write(b);
        }
        Ok(())
    }
}


#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::syscall::vprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
 () => (print!("\n"));
    ($($arg:tt)*) => ({
        $crate::syscall::vprint(format_args!($($arg)*));
        $crate::print!("\n");
    })
}

pub fn vprint(args: fmt::Arguments) {
    let mut c = Console;
    c.write_fmt(args).unwrap();
}
