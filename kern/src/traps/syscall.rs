use alloc::boxed::Box;
use core::time::Duration;

use crate::console::{kprint, kprintln, CONSOLE};
use crate::process::State;
use crate::traps::TrapFrame;
use crate::SCHEDULER;
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
    let boxed_fnmut = Box::new(move |_: &mut crate::process::Process| {
        timer::current_time() >= desired_time
    });
    SCHEDULER.switch(State::Waiting(boxed_fnmut), tf);

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

/// Kills current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    SCHEDULER.switch(State::Dead, tf);
    let id = SCHEDULER.kill(tf).expect("failed to kill proc");
    kprintln!("killed proc{}", id);
}

/// Write to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    let mut console = CONSOLE.lock();
    console.write_byte(b);
    tf.regs[7] = 1;
}

/// Returns current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.regs[0] = tf.tpidr;
    tf.regs[7] = 1;
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    match num as usize {
        NR_SLEEP => {
            sys_sleep(tf.regs[0] as u32, tf);
        },
        NR_TIME => {
            sys_time(tf);
        },
        NR_EXIT => {
            sys_exit(tf);
        },
        NR_WRITE => {
            sys_write(tf.regs[0] as u8, tf);
        },
        NR_GETPID => {
            sys_getpid(tf);
        },
        _ => {
            panic!("unimplemented syscall");
        }
    }
}
