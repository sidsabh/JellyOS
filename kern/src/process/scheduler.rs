use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::borrow::Borrow;
use core::fmt;
use core::ops::{BitAnd, BitOr};
use core::time::Duration;

use aarch64::*;

use crate::console::kprintln;
use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::{IRQ, VMM};

use pi::interrupt;
use pi::timer;

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
    pub fn start(&'static self) -> ! {
        // register handler fn for timer
        IRQ.register(
            pi::interrupt::Interrupt::Timer1,
            Box::new(|tf: &mut TrapFrame| {
                // tf was the interrupted processes' trap frame
                timer::tick_in(crate::param::TICK);
                self.switch(State::Ready, tf);

                // if let Some(id) = self.kill(tf) {
                //     kprintln!("{}", id);
                // }
            }),
        );

        // enable timer interrupts
        interrupt::Controller::new().enable(interrupt::Interrupt::Timer1);

        // set timer interupt
        timer::tick_in(crate::param::TICK);


        // run first
        // self.switch_to(&mut TrapFrame::default());

        let mut p = Process::new().expect("failed to make process");
        p.context.pc = idle_proc as *const () as *const u64 as u64;
        // p.context.pc = GlobalScheduler::switch_to as *const () as *const u64 as u64;
        p.context.sp = p.stack.top().as_u64();
        p.context.pstate |= 1 << 6; // enable IRQ exceptions
        p.context.pstate |= 0b0100; // set current EL to 0

        let frame_addr = p.context.as_ref() as *const TrapFrame as *const u64 as u64;

        unsafe {
            asm!(
                "mov SP, {context:x}",
                "bl vec_context_restore",
                "adr x0, _start",
                "mov SP, x0",
                "mov x0, xzr",
                "mov x1, {context:x}",
                "eret",
                context = in(reg) frame_addr,
            );
        }

        loop {}
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Scheduler::new());

        let mut p1 = Process::new().expect("failed to make process");
        p1.context.pc = proc1 as *const () as *const u64 as u64;
        p1.context.sp = p1.stack.top().as_u64();
        p1.context.pstate |= 1 << 6; // enable IRQ exceptions
        p1.context.pstate &= !0b1100; // set current EL to 0

        self.add(p1);

        let mut p2 = Process::new().expect("failed to make process");
        p2.context.pc = proc2 as *const () as *const u64 as u64;
        p2.context.sp = p2.stack.top().as_u64();
        p2.context.pstate |= 1 << 6; // enable IRQ exceptions
        p2.context.pstate &= !0b1100; // set current EL to 0

        // self.add(p2);
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
    processes: VecDeque<Process>, // queue
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            last_id: None,
        }
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        let new_id = self.processes.len() as u64;

        process.context.tpidr = new_id;
        self.processes.push_back(process);
        Some(new_id)
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        for (i, p) in self.processes.iter_mut().enumerate() {
            if matches!(p.state, State::Running) && p.context.tpidr == tf.tpidr {
                p.state = new_state;
                p.context = Box::new(*tf);
                let rproc = self.processes.remove(i).unwrap();
                self.processes.push_back(rproc);
                return true;
            }
        }
        false
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        for (i, p) in self.processes.iter_mut().enumerate() {
            if p.is_ready() {
                p.state = State::Running;
                let rproc = self.processes.remove(i).unwrap();
                let pid = rproc.context.tpidr;
                
                *tf = *rproc.context; // context switch bro
                self.processes.push_front(rproc);

                self.last_id = Some(pid);
                return Some(pid);
            }
        }
        None
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        for (i, p) in self.processes.iter_mut().enumerate() {
            if matches!(p.state, State::Running) && p.context.tpidr == tf.tpidr {
                p.state = State::Dead;
                let rproc = self.processes.remove(i).unwrap();
                let pid = rproc.context.tpidr;
                drop(rproc);  // Explicitly drop the process instance
                return Some(pid);
            }
        }
        None
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

extern "C" fn idle_proc() {
    loop {
        kprintln!("idle proc here");
        timer::spin_sleep(Duration::from_secs(1));
    }
}


extern "C" fn proc1() {
    shell::shell("hi");
    let mut ctr = 0;
    loop {
        kprintln!("proc1 here with {}", ctr);
        timer::spin_sleep(Duration::from_secs(1));
        ctr +=1;
    }
}


extern "C" fn proc2() {
    loop {
        kprintln!("proc2 here");
        timer::spin_sleep(Duration::from_secs(1));
    }
}
