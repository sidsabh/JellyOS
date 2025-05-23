use aarch64::{affinity, current_el};
use alloc::boxed::{self, Box};
use core::hint::spin_loop;
use core::net::Ipv4Addr;
use core::time::Duration;
use fat32::traits::FileSystem;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::kprint;
use crate::param::USER_IMG_BASE;
use crate::process::{GlobalScheduler, State};
use crate::traps::TrapFrame;
use crate::{ETHERNET, SCHEDULER};

use kernel_api::*;
use pi::timer;
use smoltcp::wire::Ipv4Address;
pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    match num as usize {
        NR_SLEEP => sys_sleep(tf.regs[0] as u32, tf),
        NR_TIME => sys_time(tf),
        NR_EXIT => sys_exit(tf),
        NR_GETPID => sys_getpid(tf),
        NR_WRITE_STR => sys_write_str(tf.regs[0] as usize, tf.regs[1] as usize, tf),
        NR_OPEN => sys_open(tf.regs[0] as usize, tf),
        NR_CLOSE => sys_close(tf.regs[0] as usize, tf),
        NR_READ => sys_read(
            tf.regs[0] as usize,
            tf.regs[1] as usize,
            tf.regs[2] as usize,
            tf,
        ),
        NR_WRITE => sys_write(
            tf.regs[0] as usize,
            tf.regs[1] as usize,
            tf.regs[2] as usize,
            tf,
        ),
        NR_SEEK => sys_seek(tf.regs[0] as usize, tf.regs[1] as usize, tf),
        NR_LEN => sys_len(tf.regs[0] as usize, tf),
        NR_READDIR => sys_readdir(
            tf.regs[0] as usize,
            tf.regs[1] as usize,
            tf.regs[2] as usize,
            tf,
        ),
        NR_EXEC => sys_exec(tf.regs[0] as usize, tf),
        NR_FORK => sys_fork(tf),
        NR_WAITPID => sys_wait(tf, tf.regs[0] as usize),
        NR_SOCK_CREATE => sys_sock_create(tf),
        NR_SOCK_STATUS => sys_sock_status(tf.regs[0] as usize, tf),
        NR_SOCK_CONNECT => sys_sock_connect(
            tf.regs[0] as usize,
            IpEndpoint {
                addr: IpAddress::Ipv4(Ipv4Address::new(
                    (tf.regs[1] >> 24) as u8,
                    (tf.regs[1] >> 16) as u8,
                    (tf.regs[1] >> 8) as u8,
                    tf.regs[1] as u8,
                )),
                port: tf.regs[2] as u16,
            },
            tf,
        ),
        NR_SOCK_LISTEN => sys_sock_listen(tf.regs[0] as usize, tf.regs[1] as u16, tf),
        NR_SOCK_SEND => sys_sock_send(
            tf.regs[0] as usize,
            tf.regs[1] as usize,
            tf.regs[2] as usize,
            tf,
        ),
        NR_SOCK_RECV => sys_sock_recv(
            tf.regs[0] as usize,
            tf.regs[1] as usize,
            tf.regs[2] as usize,
            tf,
        ),
        _ => panic!("unimplemented syscall: {}", num),
    }
}

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    trace!("Core {} is running sleep", affinity());
    let start = timer::current_time();
    let desired_time = timer::current_time() + Duration::from_millis(ms as u64);
    trace!(
        "Process {}: Sleeping for {} ms, start: {:?}, desired: {:?}\n",
        tf.tpidr,
        ms,
        start,
        desired_time
    );
    let boxed_fnmut = Box::new(move |process: &mut crate::process::Process| {
        let res = timer::current_time() >= desired_time;
        if res {
            let tf = &mut process.context;
            tf.regs[0] = (timer::current_time() - start).as_millis() as u64;
            tf.regs[7] = 1;
            trace!(
                "Process {}: Woke up, current time: {:?}",
                tf.tpidr,
                timer::current_time()
            );
        }

        res
    });

    SCHEDULER.block(State::Waiting(Some(boxed_fnmut)), tf);
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
    // get parent_semaphore
    let parent_semaphore = SCHEDULER.with_current_process_mut(tf, |process| process.parent.clone());
    if let Some(parent_semaphore) = parent_semaphore {
        // set parent semaphore
        let mut g = parent_semaphore.lock();
        g.complete();
        g.exit_code = Some(0); // TODO: add support for exit codes
    }

    // remove from scheduler
    let id = SCHEDULER.kill(tf).expect("failed to kill process");
    assert!(id == tf.tpidr);
    GlobalScheduler::idle_thread();
}

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

use crate::mutex::Mutex;
use crate::process::ChildStatus;
use alloc::sync::Arc;
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
                            handle: Arc::new(Mutex::new(Box::new(file))),
                            offset: 0,
                        }));
                    }
                    fat32::vfat::Entry::DirEntry(dir) => {
                        process.files.push(Some(crate::process::ProcessFile {
                            handle: Arc::new(Mutex::new(Box::new(dir))),
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
/// IMPORTANT: this is a blocking syscall. if the FD is the console, any other important tasks will be blocked until the syscall is complete.
pub fn sys_read(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    let handle = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return None;
        }
        let y = process.files[fd].as_mut().unwrap();
        Some(y.handle.clone()) // Clone the Arc (increases reference count)
    });

    let buf = match unsafe { to_user_slice_mut(va, len) } {
        Ok(slice) => slice,
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };
    let handle = match handle {
        Some(handle) => handle,
        None => {
            tf.regs[7] = OsError::InvalidFile as u64;
            return;
        }
    };
    let res = handle.lock().read(buf);
    match res {
        Ok(bytes) => {
            tf.regs[0] = bytes as u64; // Set the return value
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(_) => {
            tf.regs[7] = OsError::IoError as u64;
        },
    }
}

/// IMPORTANT: this is a blocking syscall. if the FD is the console, any other important tasks will be blocked until the syscall is complete.
pub fn sys_write(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    let handle = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return None;
        }
        let y = process.files[fd].as_mut().unwrap();
        Some(y.handle.clone()) // Clone the Arc (increases reference count)
    });
    let buf = match unsafe { to_user_slice(va, len) } {
        Ok(slice) => slice,
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };
    let handle = match handle {
        Some(handle) => handle,
        None => {
            tf.regs[7] = OsError::InvalidFile as u64;
            return;
        }
    };
    let res = handle.lock().write(buf);
    match res {
        Ok(bytes) => {
            tf.regs[0] = bytes as u64; // Set the return value
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(_) => {
            tf.regs[7] = OsError::IoError as u64;
        },
    }
}

pub fn sys_seek(fd: usize, offset: usize, tf: &mut TrapFrame) {
    let result = SCHEDULER.with_current_process_mut(tf, |process| {
        if fd >= process.files.len() || process.files[fd].is_none() {
            return OsError::InvalidFile as u64;
        }

        let y = process.files[fd].as_mut().unwrap();
        let handle = y.handle.clone();
        let res = handle.lock().seek(offset);
        match res {
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

        match process.files[fd].as_ref().unwrap().handle.lock().size() {
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
        if !file.handle.lock().is_dir() {
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
        let y = process.files[fd].as_mut().unwrap();
        let handle = y.handle.clone();
        let res = handle.lock().readdir(user_buffer);
        match res {
            Ok(bytes) if bytes > 0 => (OsError::Ok as u64, bytes),
            Ok(_) => (OsError::IoErrorEof as u64, 0),
            Err(_) => (OsError::InvalidDirectory as u64, 0),
        }
    });

    tf.regs[0] = bytes_read as u64;
    tf.regs[7] = result;
}

use crate::process::Process;
use shim::path::Path;
pub fn sys_exec(va: usize, tf: &mut TrapFrame) {
    trace!("[sys_exec] Received request to exec at VA: {:#x}", va);

    // Read the path string
    let path = match unsafe { to_user_slice(va, 256) } {
        Ok(slice) => core::str::from_utf8(slice).unwrap_or(""),
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };
    let clean_path = path.trim_end_matches('\0');

    // Instead of reading argv from the stack, get it from tf.regs[1]
    let argv_ptr = tf.regs[1] as usize;
    let mut args = alloc::vec![];

    if argv_ptr != 0 {
        // For example, assume the argv buffer is 256 bytes long.
        let argv_slice = match unsafe { to_user_slice(argv_ptr, 256) } {
            Ok(slice) => slice,
            Err(_) => {
                tf.regs[7] = OsError::BadAddress as u64;
                return;
            }
        };
        // Assume the first 8 bytes is argc (u64 in native endian)
        if argv_slice.len() >= 8 {
            let argc = u64::from_ne_bytes(argv_slice[0..8].try_into().unwrap()) as usize;
            // For each argument pointer (assume 8 bytes each) follow with the string.
            for i in 0..argc {
                let start = 8 + i * 8;
                let end = start + 8;
                if end > argv_slice.len() {
                    break;
                }
                let ptr_bytes: [u8; 8] = argv_slice[start..end].try_into().unwrap();
                let arg_ptr = u64::from_ne_bytes(ptr_bytes) as *const u8;
                if arg_ptr.is_null() {
                    break;
                }
                // Read the null-terminated string from user memory.
                let arg_str = unsafe {
                    let mut len = 0;
                    while core::ptr::read(arg_ptr.add(len)) != 0 {
                        len += 1;
                    }
                    core::str::from_utf8(core::slice::from_raw_parts(arg_ptr, len))
                        .unwrap_or("[Invalid UTF-8]")
                };
                // move to heap then push
                use crate::alloc::string::ToString;
                let arg_str = arg_str.to_string();
                args.push(arg_str);
            }
        }
    }

    debug!("[sys_exec] Executing: '{}'", clean_path);
    debug!("[sys_exec] Args: {:?}", args);
    debug!("core {} is running execve", affinity());

    // Run execve() and update process.context, etc.
    let new_tf = SCHEDULER.with_current_process_mut(tf, |process| {
        match Process::execve(process, Path::new(clean_path), args) {
            Ok(_) => Some(*process.context),
            Err(_) => None,
        }
    });

    trace!("[sys_exec] tf: {:#x?}", new_tf);
    match new_tf {
        Some(context) => {
            debug!("[sys_exec] Switching to user mode at {:#x}", context.pc);
            *tf = context; // Update the trap frame
                           // TLB flush happens before eret
        }
        None => {
            trace!("[sys_exec] ERROR: execve() failed!");
            tf.regs[7] = OsError::InvalidFile as u64;
        }
    }
}

pub fn sys_fork(tf: &mut TrapFrame) {
    trace!("[sys_fork] Forking process...");

    let child_fut = Arc::new(Mutex::new(ChildStatus::new()));
    let mut new_proc = SCHEDULER.with_current_process_mut(tf, |parent| {
        // Create a new process
        parent.children.push(child_fut.clone());
        parent.clone()
    });

    new_proc.state = State::Ready;
    *new_proc.context = *tf; // Updated frame
                             // print tf:
    new_proc.context.regs[0] = 0; // Child returns 0
    new_proc.context.ttbr1_el1 = new_proc.vmap.get_baddr().as_u64();
    new_proc.parent = Some(child_fut.clone());

    let id = SCHEDULER.add(new_proc); // Add the new process to the scheduler

    // Set the child process's PID
    {
        let mut g = child_fut.lock();
        g.pid = Some(id); // technically should be set before add , but whatever
    }

    // // Parent returns child PID
    tf.regs[0] = id as u64;
}

pub fn sys_wait(tf: &mut TrapFrame, pid: usize) {
    let boxed_fnmut = Box::new(move |process: &mut crate::process::Process| {
        let mut child = None;
        let mut child_done: bool = false;
        for c in process.children.iter() {
            let g = c.lock();
            if g.pid == Some(pid as u64) {
                child = Some(c.clone());
                child_done = g.done;
                break;
            }
        }

        if child.is_none() {
            process.context.regs[7] = OsError::InvalidFile as u64;
            return true;
        }
        if child_done {
            process.context.regs[0] = pid as u64;
            // process.context.regs[1] = child.exit_code.unwrap() as u64;
            process.context.regs[7] = OsError::Ok as u64;
        }

        child_done
    });

    SCHEDULER.block(State::Waiting(Some(boxed_fnmut)), tf);
}

/// socket list.
///
pub fn sys_sock_create(tf: &mut TrapFrame) {
    let handle = ETHERNET.add_socket();
    SCHEDULER.with_current_process_mut(tf, |process| {
        process.sockets.push(handle);
        process.context.regs[0] = process.sockets.len() as u64 - 1;
    });
    tf.regs[7] = OsError::Ok as u64;
    trace!("Socket created: {}", tf.regs[0]);
}

use smoltcp::socket::SocketHandle;

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
    let socket_handle: Option<SocketHandle> = SCHEDULER.with_current_process_mut(tf, |process| {
        if sock_idx >= process.sockets.len() {
            return None;
        }
        Some(process.sockets[sock_idx])
    });

    if socket_handle.is_none() {
        tf.regs[7] = OsError::InvalidSocket as u64;
        return;
    }

    let socket = socket_handle.unwrap();
    let status = ETHERNET.with_socket(socket, |s| {
        let is_active = s.is_active();
        let is_listening = s.is_listening();
        let can_send = s.can_send();
        let can_recv = s.can_recv();
        (is_active, is_listening, can_send, can_recv)
    });
    tf.regs[0] = status.0 as u64;
    tf.regs[1] = status.1 as u64;
    tf.regs[2] = status.2 as u64;
    tf.regs[3] = status.3 as u64;
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
    
    let socket_handle: Option<SocketHandle> = SCHEDULER.with_current_process_mut(tf, |process| {
        if sock_idx >= process.sockets.len() {
            return None;
        }
        Some(process.sockets[sock_idx])
    });

    if socket_handle.is_none() {
        tf.regs[7] = OsError::InvalidSocket as u64;
        return;
    }

    let socket = socket_handle.unwrap();

    let local_port = ETHERNET.get_ephemeral_port();
    if local_port.is_none() {
        tf.regs[7] = OsError::NoEntry as u64;
        return;
    }
    let local_port = local_port.unwrap();
    let local_endpoint = IpEndpoint {
        addr: IpAddress::from(Ipv4Address::new(169, 254, 32, 10)),
        port: local_port,
    };
    
    let result = ETHERNET.with_socket(socket, |s| {
        s.connect(local_endpoint, remote_endpoint)
    });

    match result {
        Ok(_) => {
            tf.regs[7] = OsError::Ok as u64;
            ETHERNET.mark_port(local_port);
        }
        Err(e) => {
            tf.regs[7] = e as u64;
        }
    }
    trace!("Socket connected: {}", tf.regs[0]);
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
    let socket_handle: Option<SocketHandle> = SCHEDULER.with_current_process_mut(tf, |process| {
        if sock_idx >= process.sockets.len() {
            return None;
        }
        Some(process.sockets[sock_idx])
    });

    if socket_handle.is_none() {
        tf.regs[7] = OsError::InvalidSocket as u64;
        return;
    }

    let socket = socket_handle.unwrap();

    let result = ETHERNET.with_socket(socket, |s| s.listen(local_port));

    match result {
        Ok(_) => {
            tf.regs[7] = OsError::Ok as u64;
            ETHERNET.mark_port(local_port);
        }
        Err(e) => {
            tf.regs[7] = e as u64;
        }
    }
    trace!("Socket listening: {}", tf.regs[0]);
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
    
    let socket_handle: Option<SocketHandle> = SCHEDULER.with_current_process_mut(tf, |process| {
        if sock_idx >= process.sockets.len() {
            return None;
        }
        Some(process.sockets[sock_idx])
    });

    if socket_handle.is_none() {
        tf.regs[7] = OsError::InvalidSocket as u64;
        return;
    }

    let socket = socket_handle.unwrap();

    // use to_user_slice(va, len) for the buffer
    let buf = match unsafe { to_user_slice(va, len) } {
        Ok(slice) => slice,
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };

    let result = ETHERNET.with_socket(socket, |s| s.send_slice(buf));
    match result {
        Ok(bytes) => {
            tf.regs[0] = bytes as u64;
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(e) => {
            tf.regs[7] = e as u64;
        }
    }
    trace!("Socket sent: {}", tf.regs[0]);

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
    let socket_handle: Option<SocketHandle> = SCHEDULER.with_current_process_mut(tf, |process| {
        if sock_idx >= process.sockets.len() {
            return None;
        }
        Some(process.sockets[sock_idx])
    });

    if socket_handle.is_none() {
        tf.regs[7] = OsError::InvalidSocket as u64;
        return;
    }

    let socket = socket_handle.unwrap();

    // use to_user_slice(va, len) for the buffer
    let buf = match unsafe { to_user_slice_mut(va, len) } {
        Ok(slice) => slice,
        Err(_) => {
            tf.regs[7] = OsError::BadAddress as u64;
            return;
        }
    };

    let result = ETHERNET.with_socket(socket, |s| s.recv_slice(buf));
    match result {
        Ok(bytes) => {
            tf.regs[0] = bytes as u64;
            tf.regs[7] = OsError::Ok as u64;
        }
        Err(e) => {
            tf.regs[7] = e as u64;
        }
    }
    trace!("Socket received: {}", tf.regs[0]);
}


