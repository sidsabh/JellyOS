use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::fmt;
use core::ops::{BitAnd, BitOr};

use aarch64::*;

use crate::console::kprintln;
use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::{IRQ, VMM};

use pi::timer;
use pi::interrupt;

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enter a critical region and execute the provided closure with the
    /// internal scheduler.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }

    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(move |scheduler| scheduler.add(process))
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfe();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentaion on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }



    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&self) -> ! {

        
        // enable timer interrupts
        interrupt::Controller::new().enable(interrupt::Interrupt::Timer1);

        // register handler fn for timer
        IRQ.register(pi::interrupt::Interrupt::Timer1, Box::new(timer_handler));

        // set timer interupt
        timer::tick_in(crate::param::TICK);

        let mut p = Process::new().expect("failed to make process");
        p.context.pc = run_shell as *const () as *const u64 as u64;
        p.context.sp = p.stack.top().as_u64();
        p.context.pstate |= 1 << 7; // enable IRQ exceptions
        p.context.pstate &= !0b1100; // set current EL to 0

        let frame_addr = p.context.as_ref() as *const TrapFrame as *const u64 as u64;

        unsafe {
            asm!(
                "mov SP, {context:x}",
                "bl context_restore",
                "adr x0, _start",
                "mov SP, x0",
                "mov x0, xzr",
                "eret",
                context = in(reg) frame_addr
            );
        }

        loop {}
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        unimplemented!("GlobalScheduler::initialize()")
    }

    // The following method may be useful for testing Phase 3:
    //
    // * A method to load a extern function to the user process's page table.
    //
    // pub fn test_phase_3(&self, proc: &mut Process){
    //     use crate::vm::{VirtualAddr, PagePerm};
    //
    //     let mut page = proc.vmap.alloc(
    //         VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);
    //
    //     let text = unsafe {
    //         core::slice::from_raw_parts(test_user_process as *const u8, 24)
    //     };
    //
    //     page[0..24].copy_from_slice(text);
    // }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        unimplemented!("Scheduler::new()")
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        unimplemented!("Scheduler::add()")
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        unimplemented!("Scheduler::schedule_out()")
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        unimplemented!("Scheduler::switch_to()")
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        unimplemented!("Scheduler::kill()")
    }
}

use core::arch::asm;

pub extern "C" fn test_user_process() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!(
                "mov x0, {ms:x}",
                "svc {svc_num}",
                "mov {ems}, x0",
                "mov {error}, x7",
                ms = in(reg) ms,
                svc_num = const 1,
                ems = out(reg) elapsed_ms,
                error = out(reg) error,
                out("x0") _,   // Clobbers x0
                out("x7") _,   // Clobbers x7
                options(nostack),
            );
        }

        // You might want to add some logic here to do something with `elapsed_ms` and `error`
    }
}

use crate::shell;
use aarch64::current_el;
extern "C" fn run_shell() {
    // unsafe { asm!("brk 1"); }
    // unsafe { asm!("brk 2"); }

    // won't work until we enable increasing levels
    // let mut value: u64;
    // unsafe {
    //     asm!(
    //         "mrs {value}, DAIF",
    //         value = out(reg) value,
    //     );
    // }
    // kprintln!("daif: {}", value);


    // shell::shell("user0> ");
    unsafe { asm!("brk 3"); }
    loop { shell::shell("user1> "); }
}


fn timer_handler(tf : &mut TrapFrame) {
    kprintln!("got timer interrupted with tf {:#?}", tf);
    timer::tick_in(crate::param::TICK);
}