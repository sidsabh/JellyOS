use aarch64::{affinity, current_el};
use alloc::boxed::Box;
use fat32::traits::FileSystem;
use core::time::Duration;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::param::USER_IMG_BASE;
use crate::process::State;
use crate::traps::TrapFrame;
use crate::{ETHERNET, SCHEDULER};

use kernel_api::*;
use pi::timer;

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let start = timer::current_time();
    let desired_time = timer::current_time()+Duration::from_millis(ms as u64);
    kprint!("Sleeping for {} ms, start: {:?}, desired: {:?}\n", ms, start, desired_time);
    let boxed_fnmut = Box::new(move |_: &mut crate::process::Process| {
        timer::current_time() >= desired_time
    });
    SCHEDULER.block(State::Waiting(Some(boxed_fnmut)), tf);

    tf.regs[0] = (timer::current_time() - start).as_millis() as u64;
    tf.regs[7] = 1;
}

/// Returns current time.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns two
/// parameter:
///  - current time as seconds
///  - fractional part of the current time, in nanoseconds.
pub fn sys_time(tf: &mut TrapFrame) {
    tf.regs[0] = timer::current_time().as_secs() as u64;
    tf.regs[1] = timer::current_time().subsec_nanos() as u64;
    tf.regs[7] = 1;
}

/// Kills the current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    let id = SCHEDULER.kill(tf).expect("failed to kill proc {}");
    //kprintln!("killed proc {}", id);
    assert!(id == tf.tpidr);
    //kprintln!("{:#?}", SCHEDULER);
    while SCHEDULER.switch_to(tf) == u64::MAX {
        aarch64::wfi();
    }
    //kprintln!("tf: {:x}", tf.sp, tf.pc, tf.tpidr);
}

/// Writes to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
// pub fn sys_write(b: u8, tf: &mut TrapFrame) {
//     let mut console = CONSOLE.lock();
//     use shim::io::Write;
//     console.write(&mut[b]).expect("write failed");
//     tf.regs[7] = 1;
// }

/// Returns the current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.regs[0] = tf.tpidr;
    tf.regs[7] = 1;
}

/// Returns a slice from a virtual address and a legnth.
///
/// # Errors
/// This functions returns `Err(OsError::BadAddress)` if the slice is not entirely
/// in userspace.
unsafe fn to_user_slice<'a>(va: usize, len: usize) -> OsResult<&'a [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts(va as *const u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}
/// Returns a mutable slice from a virtual address and a legnth.
///
/// # Errors
/// This functions returns `Err(OsError::BadAddress)` if the slice is not entirely
/// in userspace.
unsafe fn to_user_slice_mut<'a>(va: usize, len: usize) -> OsResult<&'a mut [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts_mut(va as *mut u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}
/// Writes a UTF-8 string to the console.
///
/// This system call takes the address of the buffer as the first parameter and
/// the length of the buffer as the second parameter.
///
/// In addition to the usual status value, this system call returns the length
/// of the UTF-8 message.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::InvalidArgument`: The provided buffer is not UTF-8 encoded.
pub fn sys_write_str(va: usize, len: usize, tf: &mut TrapFrame) {
    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));

    match result {
        Ok(msg) => {
            kprint!("{}", msg);

            tf.regs[0] = msg.len() as u64; // sorry for commenting you sir
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(e) => {
            tf.regs[7] = e as u64;
        }
    }
}
/// Sends data with a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes sent.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `send_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_send(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_send")
}

/// Receives data from a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes read.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `recv_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_recv(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_recv")
}

/// socket list.
///
/// This function does neither take any parameter nor return anything,
/// except the usual return code that indicates successful syscall execution.
pub fn sys_sock_create(tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_create")
}

/// Returns the status of a socket.
///
/// This system call takes a socket descriptor as the first parameter.
///
/// In addition to the usual status value, this system call returns four boolean
/// values that describes the status of the queried socket.
///
/// - x0: is_active
/// - x1: is_listening
/// - x2: can_send
/// - x3: can_recv
///
/// # Errors
/// This function returns `OsError::InvalidSocket` if a socket that corresponds
/// to the provided descriptor is not found.
pub fn sys_sock_status(sock_idx: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_status")
}

/// Connects a local ephemeral port to a remote IP endpoint with a socket.
///
/// This system call takes a socket descriptor as the first parameter, the IP
/// of the remote endpoint as the second paramter in big endian, and the port
/// number of the remote endpoint as the third parameter.
///
/// `handle_syscall` should read the value of registers and create a struct that
/// implements `Into<IpEndpoint>` when calling this function.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::NoEntry`: Fails to allocate an ephemeral port
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `connect()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `connect()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `connect()`.
pub fn sys_sock_connect(
    sock_idx: usize,
    remote_endpoint: impl Into<IpEndpoint>,
    tf: &mut TrapFrame,
) {
    // Lab 5 2.D
    unimplemented!("sys_sock_connect")
}

/// Listens on a local port for an inbound connection.
///
/// This system call takes a socket descriptor as the first parameter and the
/// local ports to listen on as the second parameter.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `listen()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `listen()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `listen()`.
pub fn sys_sock_listen(sock_idx: usize, local_port: u16, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_listen")
}


pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    match num as usize {
        NR_SLEEP => sys_sleep(tf.regs[0] as u32, tf),
        NR_TIME => sys_time(tf),
        NR_EXIT => sys_exit(tf),
        NR_GETPID => sys_getpid(tf),
        NR_WRITE_STR => sys_write_str(tf.regs[0] as usize, tf.regs[1] as usize, tf),
        NR_OPEN => sys_open(tf.regs[0] as usize, tf),
        NR_CLOSE => sys_close(tf.regs[0] as usize, tf),
        NR_READ => sys_read(tf.regs[0] as usize, tf.regs[1] as usize, tf.regs[2] as usize, tf),
        NR_WRITE => sys_write(tf.regs[0] as usize, tf.regs[1] as usize, tf.regs[2] as usize, tf),
        NR_SEEK => sys_seek(tf.regs[0] as usize, tf.regs[1] as usize, tf),
        NR_LEN => sys_len(tf.regs[0] as usize, tf),
        NR_READDIR => sys_readdir(tf.regs[0] as usize, tf.regs[1] as usize, tf.regs[2] as usize, tf),
        NR_EXEC => sys_exec(tf.regs[0] as usize, tf),
        NR_FORK => sys_fork(tf),
        _ => panic!("unimplemented syscall: {}", num),
    }
}

pub fn sys_open(va: usize, tf: &mut TrapFrame) {

    let path = match unsafe { to_user_slice(va, 256) } {
        Ok(slice) => {
            let s = core::str::from_utf8(slice.split(|&c| c == 0).next().unwrap_or(&[]))
                .unwrap_or("[invalid utf8]");
            s
        }
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };

    if path.is_empty() {
        tf.regs[7] = OsError::NoEntry as u64;
        return;
    }

    match crate::FILESYSTEM.open(path) {
        Ok(entry) => {
            let fd = SCHEDULER.with_current_process_mut(tf, |process| {
                let fd = process.files.len();
                match entry {
                    fat32::vfat::Entry::FileEntry(file) => {
                        process.files.push(Some(crate::process::ProcessFile {
                            handle: Box::new(file),
                            offset: 0,
                        }));
                    }
                    fat32::vfat::Entry::DirEntry(dir) => {
                        process.files.push(Some(crate::process::ProcessFile {
                            handle: Box::new(dir),
                            offset: 0,
                        }));
                    }
                }
                fd
            });

            tf.regs[0] = fd as u64;
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(_) => {
            tf.regs[7] = OsError::NoEntry as u64;
        }
    }
}




pub fn sys_close(fd: usize, tf: &mut TrapFrame) {
    let result = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return OsError::InvalidFile as u64;
        }
        process.files[fd] = None; // Remove file handle
        OsError::Ok as u64
    });

    tf.regs[7] = result;
}


pub fn sys_read(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    let (result, bytes_read) = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return (OsError::InvalidFile as u64, 0);
        }

        let buf = match unsafe { to_user_slice_mut(va, len) } {
            Ok(slice) => slice,
            Err(_) => return (OsError::BadAddress as u64, 0),
        };

        match process.files[fd].as_mut().unwrap().handle.read(buf) {
            Ok(bytes) => (OsError::Ok as u64, bytes),
            Err(_) => (OsError::IoError as u64, 0),
        }
    });

    tf.regs[0] = bytes_read as u64;  // Set the return value **after** the closure
    tf.regs[7] = result;
}

pub fn sys_write(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    let (result, bytes_written) = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return (OsError::InvalidFile as u64, 0);
        }

        let buf = match unsafe { to_user_slice(va, len) } {
            Ok(slice) => slice,
            Err(_) => return (OsError::BadAddress as u64, 0),
        };

        match process.files[fd].as_mut().unwrap().handle.write(buf) {
            Ok(bytes) => (OsError::Ok as u64, bytes),
            Err(_) => (OsError::IoError as u64, 0),
        }
    });

    tf.regs[0] = bytes_written as u64;
    tf.regs[7] = result;
}


pub fn sys_seek(fd: usize, offset: usize, tf: &mut TrapFrame) {
    let result = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return OsError::InvalidFile as u64;
        }

        match process.files[fd].as_mut().unwrap().handle.seek(offset) {
            Ok(_) => OsError::Ok as u64,
            Err(_) => OsError::IoError as u64,
        }
    });

    tf.regs[7] = result;
}


pub fn sys_len(fd: usize, tf: &mut TrapFrame) {
    let (result, size) = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return (OsError::InvalidFile as u64, 0);
        }

        match process.files[fd].as_ref().unwrap().handle.size() {
            Some(len) => (OsError::Ok as u64, len),
            None => (OsError::IoError as u64, 0),
        }
    });

    tf.regs[0] = size as u64;
    tf.regs[7] = result;
}

pub fn sys_readdir(fd: usize, user_buf: usize, buf_len: usize, tf: &mut TrapFrame) {

    let (result, bytes_read) = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return (OsError::InvalidFile as u64, 0);
        }

        let file = process.files[fd].as_mut().unwrap();

        // Ensure the file is actually a directory
        if !file.handle.is_dir() {
            return (OsError::InvalidDirectory as u64, 0);
        }

        // Validate user-space buffer before writing to it
        let user_buffer = match unsafe { to_user_slice_mut(user_buf, buf_len) } {
            Ok(buf) => buf,
            Err(_) => {
                return (OsError::BadAddress as u64, 0);
            }
        };

        // Read directory entries into user buffer
        match file.handle.readdir(user_buffer) {
            Ok(bytes) if bytes > 0 => {
                (OsError::Ok as u64, bytes)
            }
            Ok(_) => {
                (OsError::IoErrorEof as u64, 0)
            }
            Err(_) => {
                (OsError::InvalidDirectory as u64, 0)
            }
        }
    });

    tf.regs[0] = bytes_read as u64;
    tf.regs[7] = result;
}

use crate::process::Process;
use shim::path::Path;
pub fn sys_exec(va: usize, tf: &mut TrapFrame) {
    kprintln!("[sys_exec] Received request to exec at VA: {:#x}", va);

    let path = match unsafe { to_user_slice(va, 256) } {
        Ok(slice) => core::str::from_utf8(slice).unwrap_or(""),
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };

    kprintln!("[sys_exec] Executing: {}", path);

    match Process::load(Path::new(path)) {
        Ok(mut process) => {
            kprintln!("[sys_exec] Process loaded successfully!");

            // delete old process
            let old_pid = SCHEDULER.kill(tf).expect("failed to kill proc {}");
            assert!(old_pid == tf.tpidr);

            // switch to new process
            process.state = State::Running; // Set state to running
            *tf = *process.context;  
        }
        Err(_) => {
            kprintln!("[sys_exec] Failed to load process: '{}'", path);
            tf.regs[7] = OsError::NoEntry as u64;
        }
    }
}


pub fn sys_fork(tf: &mut TrapFrame) {
    kprintln!("[sys_fork] Forking process...");

    let new_pid = SCHEDULER.with_current_process_mut(tf, |parent| {
        let mut new_proc = parent.clone(); // Clone the parent process

        kprintln!("[sys_fork] Cloned a process");

        new_proc.state = State::Ready;
        new_proc.context.regs[0] = 0; // Child returns 0
        new_proc.context.ttbr1_el1 = new_proc.vmap.get_baddr().as_u64();

        let pid = SCHEDULER.add(new_proc).unwrap(); // Add the new process to the scheduler

        kprintln!("[sys_fork] Created child process with PID: {}", pid);

        pid
    });

    // Parent returns child PID
    tf.regs[0] = new_pid as u64;
}
