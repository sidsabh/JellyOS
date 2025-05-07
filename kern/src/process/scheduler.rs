use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::ffi::c_void;
use core::fmt;
use core::u64;
use core::arch::asm;
use aarch64::*;
use pi::local_interrupt::LocalInterrupt;

use crate::mutex::Mutex;
use crate::net::uspi::TKernelTimerHandle;
use crate::process::{Id, Process, State};
use crate::traps::irq::IrqHandlerRegistry;
use crate::traps::TrapFrame;
use crate::GLOBAL_IRQ;
use crate::{ETHERNET, USB};

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Box<Scheduler>>>);


impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    pub fn idle_thread() -> ! {
        loop {
            unsafe {
                asm!(
                    "mov x0, {max:x}",
                    "msr tpidr_el0, x0",
                    max = in(reg) u64::MAX,
                );
                SP.set(crate::param::KERN_STACK_BASE - crate::param::KERN_STACK_SIZE * MPIDR_EL1.get_value(MPIDR_EL1::Aff0) as usize);
            }
            trace!("Idle thread running on core {}", aarch64::affinity());
            unsafe {
                sti();
            } // enable IRQs for the nap
            wfi(); // sleep until *next* interrupt
        }
    }

    /// Enters a critical region and execute the provided closure with a mutable
    /// reference to the inner scheduler.
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

    pub fn with_current_process_mut<F, R>(&self, tf: &TrapFrame, f: F) -> R
    where
        F: FnOnce(&mut Process) -> R,
    {
        self.critical(|scheduler| {
            let process = scheduler
                .find_process(tf)
                .expect("No running process found");
            f(process)
        })
    }
    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        trace!("Switch called");

        let og_id = tf.tpidr;
        if !tf.is_idle() {
            let mut old_tf = tf.clone();
            let id = self.switch_to(tf);
            if id != u64::MAX {
                self.critical(|scheduler| {
                    scheduler.schedule_out(new_state, &mut old_tf);
                });
            }

            // print stack pointer:
            let sp: u64;
            unsafe {
                asm!(
                    "mov {sp:x}, sp",
                    sp = out(reg) sp,
                    options(nomem, nostack, preserves_flags)
                );
            }
            trace!("SP: {:x}", sp);
            trace!(
                "[CORE {}] Switching from process {} to process {}",
                affinity(),
                og_id,
                id
            );
            id
        } else {
            let id = self.switch_to(tf);
            trace!(
                "[IDLE] [CORE {}] Switching from process {} to process {}",
                affinity(),
                og_id,
                id
            );
            id
        }
    }

    pub fn block(&self, new_state: State, tf: &mut TrapFrame) {
        assert!(!tf.is_idle());

        let mut old_tf = tf.clone();
        let id = self.switch_to(tf);

        self.critical(|scheduler| {
            scheduler.schedule_out(new_state, &mut old_tf);
        });

        trace!("Switching from process {} to process {}", tf.tpidr, id);
        // print tf:
        trace!("{:?}", tf);

        if id != u64::MAX {
            return;
        } else {
            info!("No process to switch to, switching to idle");
            Self::idle_thread();
        }
    }

    /// Edited to fix deadlock
    /// For more details, see the documentation on `Scheduler::switch_to()`.
    ///
    /// Returns the process's ID when a ready process is found.
    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
        if let Some(id) = rtn {
            trace!(
                "[core-{}] switch_to {:?}, pc: {:x}, lr: {:x}",
                affinity(),
                id,
                tf.pc,
                tf.regs[27]
            );
            return id;
        } else {
            return u64::MAX;
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&'static self) -> ! {
        // register handler fn for timer
        if aarch64::affinity() == 0 {
            self.initialize_global_timer_interrupt();
        }
        self.initialize_local_timer_interrupt();

        Self::idle_thread();
    }

    /// # Lab 4
    /// Initializes the global timer interrupt with `pi::timer`. The timer
    /// should be configured in a way that `Timer1` interrupt fires every
    /// `TICK` duration, which is defined in `param.rs`.
    ///
    /// # Lab 5
    /// Registers a timer handler with `Usb::start_kernel_timer` which will
    /// invoke `poll_ethernet` after 1 second.
    pub fn initialize_global_timer_interrupt(&'static self) {}

    /// Initializes the per-core local timer interrupt with `pi::local_interrupt`.
    /// The timer should be configured in a way that `CntpnsIrq` interrupt fires
    /// every `TICK` duration, which is defined in `param.rs`.
    pub fn initialize_local_timer_interrupt(&'static self) {
        // Lab 5 2.C
        use crate::IRQ;
        use pi::interrupt::Interrupt;
        IRQ.register(
            LocalInterrupt::CntPnsIrq,
            Box::new(|tf: &mut TrapFrame| {
                trace!("Timer interrupt on core {}", aarch64::affinity());

                // debug!("Is idle? {}", tf.is_idle());
                // tf was the interrupted processes' trap frame
                pi::local_interrupt::local_tick_in(aarch64::affinity(), crate::param::TICK);
                self.switch(State::Ready, tf); // context switch

                // // print current EL
                // let mut current_el: u64;
                // unsafe {
                //     asm!(
                //         "mrs {current_el}, CurrentEL",
                //         current_el = out(reg) current_el,
                //         options(nomem, nostack, preserves_flags)
                //     );
                // }
                // current_el >>= 2; // shift right to get the current EL
                // debug!("Current EL: {:x}", current_el);

                // assert!(current_el == 1, "Current EL is not EL1");

                // // print SPSEL
                // let mut spsel: u64;
                // unsafe {
                //     asm!(
                //         "mrs {spsel}, SPSel",
                //         spsel = out(reg) spsel,
                //         options(nomem, nostack, preserves_flags)
                //     );
                // }
                // debug!("SPSel: {:x}", spsel);

                // print SP_EL1
                // let sp_el1 = SP.get();
                // debug!("SP_EL1 = {:#x}", sp_el1);
            }),
        );

        // enable timer interrupts
        pi::local_interrupt::LocalController::new(aarch64::affinity()).enable_local_timer();

        // set timer interupt
        pi::local_interrupt::local_tick_in(aarch64::affinity(), crate::param::TICK);
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler.
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Box::new(Scheduler::new()));

        use shim::path::Path;
        let p = Process::load(Path::new("/programs/shell.bin"), None)
            .expect("failed to load test proc");
        self.add(p);
    }
}

/// Poll the ethernet driver and re-register a timer handler using
/// `Usb::start_kernel_timer`.
extern "C" fn poll_ethernet(_: TKernelTimerHandle, _: *mut c_void, _: *mut c_void) {
    // Lab 5 2.B
    unimplemented!("poll_ethernet")
}

/// Internal scheduler struct which is not thread-safe.
pub struct Scheduler {
    processes: VecDeque<Process>, // queue
    last_id: Id,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            last_id: 0,
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
        let new_id = self.last_id;
        self.last_id += 1;
        // let new_id = self.processes.len() as u64;
        //kprint!("{}", self.processes.len());

        debug!("Adding process with ID {}", new_id);

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

                return Some(pid);
            }
        }
        None
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Releases all process resources held by the process,
    /// removes the dead process from the queue, drops the dead process's
    /// instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        for (i, p) in self.processes.iter_mut().enumerate() {
            if p.context.tpidr == tf.tpidr {
                p.state = State::Dead;
                let rproc = self.processes.remove(i).unwrap();
                let pid = rproc.context.tpidr;
                drop(rproc); // Explicitly drop the process instance
                return Some(pid);
            }
        }
        None
    }

    /// Releases all process resources held by the current process such as sockets.
    fn release_process_resources(&mut self, tf: &mut TrapFrame) {
        // Lab 5 2.C
        unimplemented!("release_process_resources")
    }

    /// Finds a process corresponding with tpidr saved in a trap frame.
    /// Panics if the search fails.
    pub fn find_process(&mut self, tf: &TrapFrame) -> Option<&mut Process> {
        for i in 0..self.processes.len() {
            if self.processes[i].context.tpidr == tf.tpidr {
                return Some(&mut self.processes[i]);
            }
        }
        None
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.processes.len();
        write!(f, "  [Scheduler] {} processes in the queue\n", len)?;
        for i in 0..len {
            write!(
                f,
                "    queue[{}]: proc({:3})-{:?} \n",
                i, self.processes[i].context.tpidr, self.processes[i].state
            )?;
        }
        Ok(())
    }
}
