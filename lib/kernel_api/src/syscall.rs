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


pub fn sock_create() -> SocketDescriptor {
    
    // Lab 5 2.D
    let mut ecode: u64;
    let mut sockfd: u64;

    unsafe {
        asm!(
            "svc {nr_sock_create}",
            "mov {sockfd}, x0",
            "mov {ecode}, x7",
            nr_sock_create = const NR_SOCK_CREATE,
            sockfd = out(reg) sockfd,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }

    SocketDescriptor(sockfd)
}

pub fn sock_status(descriptor: SocketDescriptor) -> OsResult<SocketStatus> {
    
    let mut ecode: u64;
    let mut is_active: u64;
    let mut is_listening: u64;
    let mut can_send: u64;
    let mut can_recv: u64;

    unsafe {
        asm!(
            "mov x0, {descriptor}",
            "svc {nr_sock_status}",
            "mov {is_active}, x0",
            "mov {is_listening}, x1",
            "mov {can_send}, x2",
            "mov {can_recv}, x3",
            "mov {ecode}, x7",
            descriptor = in(reg) descriptor.0,
            nr_sock_status = const NR_SOCK_STATUS,
            is_active = out(reg) is_active,
            is_listening = out(reg) is_listening,
            can_send = out(reg) can_send,
            can_recv = out(reg) can_recv,
            ecode = out(reg) ecode,
        );
    }
    let _ = OsError::from(ecode);
    let is_active = is_active != 0;
    let is_listening = is_listening != 0;
    let can_send = can_send != 0;
    let can_recv = can_recv != 0;
    let status = SocketStatus {
        is_active,
        is_listening,
        can_send,
        can_recv,
    };
    Ok(status)

}

pub fn sock_connect(descriptor: SocketDescriptor, addr: IpAddr) -> OsResult<()> {
    let mut ecode: u64;
    unsafe {
        asm!(
            "mov x0, {descriptor}",
            "mov x1, {addr:x}",
            "mov x2, {port:x}",
            "svc {nr_sock_connect}",
            "mov {ecode}, x7",
            descriptor = in(reg) descriptor.0,
            addr = in(reg) addr.ip,
            port = in(reg) addr.port,
            nr_sock_connect = const NR_SOCK_CONNECT,
            ecode = out(reg) ecode,
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }
    err_or!(ecode, ())
}

pub fn sock_listen(descriptor: SocketDescriptor, local_port: u16) -> OsResult<()> {
    let mut ecode: u64;
    unsafe {
        asm!(
            "mov x0, {descriptor}",
            "mov x1, {local_port:x}",
            "svc {nr_sock_listen}",
            "mov {ecode}, x7",
            descriptor = in(reg) descriptor.0,
            local_port = in(reg) local_port,
            nr_sock_listen = const NR_SOCK_LISTEN,
            ecode = out(reg) ecode,
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }
    err_or!(ecode, ())
}

pub fn sock_send(descriptor: SocketDescriptor, buf: &[u8]) -> OsResult<usize> {
    let mut ecode: u64;
    let mut bytes_sent: u64;
    unsafe {
        asm!(
            "mov x0, {descriptor}",
            "mov x1, {buf_addr}",
            "mov x2, {buf_len}",
            "svc {nr_sock_send}",
            "mov {bytes_sent}, x0",
            "mov {ecode}, x7",
            descriptor = in(reg) descriptor.0,
            buf_addr = in(reg) buf.as_ptr(),
            buf_len = in(reg) buf.len(),
            nr_sock_send = const NR_SOCK_SEND,
            bytes_sent = out(reg) bytes_sent,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }
    err_or!(ecode, bytes_sent as usize)
}

pub fn sock_recv(descriptor: SocketDescriptor, buf: &mut [u8]) -> OsResult<usize> {
    let mut ecode: u64;
    let mut bytes_received: u64;
    unsafe {
        asm!(
            "mov x0, {descriptor}",
            "mov x1, {buf_addr}",
            "mov x2, {buf_len}",
            "svc {nr_sock_recv}",
            "mov {bytes_received}, x0",
            "mov {ecode}, x7",
            descriptor = in(reg) descriptor.0,
            buf_addr = in(reg) buf.as_ptr(),
            buf_len = in(reg) buf.len(),
            nr_sock_recv = const NR_SOCK_RECV,
            bytes_received = out(reg) bytes_received,
            ecode = out(reg) ecode,
            out("x0") _,   // Clobbers x0
            out("x7") _,   // Clobbers x7
            options(nostack),
        );
    }
    err_or!(ecode, bytes_received as usize)
}
