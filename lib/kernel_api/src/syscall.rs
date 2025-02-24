use core::{fmt, str};
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

// pub fn write(b: u8) {
//     let mut ecode: u64;

//     unsafe {
//         asm!(
//             "mov w0, {b:w}",
//             "svc {nr_write}",
//             "mov {ecode}, x7",
//             b = in(reg) b,
//             nr_write = const NR_WRITE,
//             ecode = out(reg) ecode,
//             out("x7") _,   // Clobbers x0
//             options(nostack),
//         );
//     }

//     let _ = OsError::from(ecode);
// }

pub fn write_str(msg: &str) {
    let mut ecode: u64;
    let mut printed_len: u64;

    unsafe {
        asm!(
            "mov x0, {str_addr:x}",
            "mov x1, {str_len:x}",
            "svc {nr_write_str}",
            "mov {printed_len:x}, x0",
            "mov {ecode:x}, x7",
            str_addr = in(reg) msg as *const str as *const usize as usize,
            str_len = in(reg) msg.len(),
            nr_write_str = const NR_WRITE_STR,
            ecode = out(reg) ecode,
            printed_len = out(reg) printed_len,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x0
            options(nostack),
        );
    }
    assert!(msg.len() == printed_len as usize); // why does this fail if i don't include??

    let _ = OsError::from(ecode);

}

pub fn getpid() -> u64 {
    let mut ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!(
            "svc {nr_getpid}",
            "mov {pid}, x0",
            "mov {ecode}, x7",
            nr_getpid = const NR_GETPID,
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


pub fn write(fd: usize, buf: &[u8]) -> OsResult<usize> {
    let mut ecode: u64;
    let mut bytes_written: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "mov x1, {buf_addr}",
            "mov x2, {buf_len}",
            "svc {nr_write}",
            "mov {bytes_written}, x0",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            buf_addr = in(reg) buf.as_ptr(),
            buf_len = in(reg) buf.len(),
            nr_write = const NR_WRITE,
            bytes_written = out(reg) bytes_written,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, bytes_written as usize)
}

pub fn open(path: &str) -> OsResult<usize> {
    let mut ecode: u64;
    let mut fd: u64;
    let mut buf = [0u8; 256];

    // Ensure the path is null-terminated
    let len = path.len().min(255);  // Prevent buffer overflow
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;  // Null-terminate the string

    unsafe {
        asm!(
            "mov x0, {path_addr}",
            "svc {nr_open}",
            "mov {fd}, x0",
            "mov {ecode}, x7",
            path_addr = in(reg) buf.as_ptr(),
            nr_open = const NR_OPEN,
            fd = out(reg) fd,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, fd as usize)
}


pub fn close(fd: usize) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "svc {nr_close}",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            nr_close = const NR_CLOSE,
            ecode = out(reg) ecode,
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, ())
}

pub fn read(fd: usize, buf: &mut [u8]) -> OsResult<usize> {
    let mut ecode: u64;
    let mut bytes_read: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "mov x1, {buf_addr}",
            "mov x2, {buf_len}",
            "svc {nr_read}",
            "mov {bytes_read}, x0",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            buf_addr = in(reg) buf.as_mut_ptr(),
            buf_len = in(reg) buf.len(),
            nr_read = const NR_READ,
            bytes_read = out(reg) bytes_read,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, bytes_read as usize)
}

pub fn seek(fd: usize, offset: usize) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "mov x1, {offset}",
            "svc {nr_seek}",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            offset = in(reg) offset,
            nr_seek = const NR_SEEK,
            ecode = out(reg) ecode,
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, ())
}

pub fn len(fd: usize) -> OsResult<usize> {
    let mut ecode: u64;
    let mut size: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "svc {nr_len}",
            "mov {size}, x0",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            nr_len = const NR_LEN,
            size = out(reg) size,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, size as usize)
}


pub fn readdir(fd: usize, buf: &mut [u8]) -> OsResult<usize> {
    let mut ecode: u64;
    let mut bytes_read: u64;

    unsafe {
        asm!(
            "mov x0, {fd}",
            "mov x1, {buf_addr}",
            "mov x2, {buf_len}",
            "svc {nr_readdir}",
            "mov {bytes_read}, x0",
            "mov {ecode}, x7",
            fd = in(reg) fd,
            buf_addr = in(reg) buf.as_mut_ptr(),
            buf_len = in(reg) buf.len(),
            nr_readdir = const NR_READDIR,
            bytes_read = out(reg) bytes_read,
            ecode = out(reg) ecode,
            out("x0") _, // Clobber registers
            out("x7") _, 
            options(nostack),
        );
    }

    err_or!(ecode, bytes_read as usize)
}


pub fn fork() -> OsResult<usize> {
    let mut ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!(
            "svc {nr_fork}",
            "mov {pid}, x0",
            "mov {ecode}, x7",
            nr_fork = const NR_FORK,
            pid = out(reg) pid,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    Ok(pid as usize) // TODO: not good error code
}

pub fn exec(path: &str, argv: &[&str]) -> OsResult<()> {
    let mut ecode: u64;
    let mut path_buf = [0u8; 256]; // Ensure buffer size is large enough

    // Copy and null-terminate the path string
    let len = path.len().min(255);
    path_buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    path_buf[len] = 0; // Null-terminate

    // Allocate space for argv pointers
    let mut argv_buf: [u64; 64] = [0; 64]; // Max 64 arguments
    let mut str_buf = [0u8; 1024]; // Store all arguments in a buffer
    let mut str_offset = 0;

    for (i, arg) in argv.iter().enumerate() {
        if i >= argv_buf.len() - 1 {
            break; // Prevent overflow
        }

        let arg_len = arg.len().min(255);
        let arg_ptr = str_buf.as_ptr() as usize + str_offset;
        
        // Copy argument into the buffer
        str_buf[str_offset..str_offset + arg_len].copy_from_slice(arg.as_bytes());
        str_buf[str_offset + arg_len] = 0; // Null-terminate

        argv_buf[i] = arg_ptr as u64;
        str_offset += arg_len + 1;
    }

    // Null-terminate the argv list
    argv_buf[argv.len()] = 0;

    unsafe {
        asm!(
            "mov x0, {path_addr}",
            "mov x1, {argv_addr}",
            "svc {nr_exec}",
            "mov {ecode}, x7",
            path_addr = in(reg) path_buf.as_ptr(),
            argv_addr = in(reg) argv_buf.as_ptr(),
            nr_exec = const NR_EXEC,
            ecode = out(reg) ecode,
            out("x7") _, // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, ())
}


pub fn wait(pid: usize) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!(
            "mov x0, {pid}",
            "svc {nr_wait}",
            "mov {ecode}, x7",
            pid = in(reg) pid,
            nr_wait = const NR_WAITPID,
            ecode = out(reg) ecode,
            out("x7") _, // Clobbers x7
            options(nostack),
        );
    }

    err_or!(ecode, ())
}
