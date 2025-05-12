pub mod frame;
mod syndrome;
mod syscall;

pub mod irq;
use crate::process::GlobalScheduler;

pub use self::frame::TrapFrame;

use crate::{FIQ, GLOBAL_IRQ, SCHEDULER};
use aarch64::HCR_EL2::TPC;
use aarch64::{affinity, current_el, FAR_EL1};
use pi::interrupt::{Controller, Interrupt};
use pi::local_interrupt::{LocalController, LocalInterrupt};

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;
use crate::percore::{self, local_irq};
use crate::traps::irq::IrqHandlerRegistry;

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Kind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Source {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Info {
    source: Source,
    kind: Kind,
}
/// This function is called when an exception occurs. The `info` parameter
/// specifies the source and kind of exception that has occurred. The `esr` is
/// the value of the exception syndrome register. Finally, `tf` is a pointer to
/// the trap frame for the exception.
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame) {

    trace!("info: {:#?}", info);
    match info.kind {
        Kind::Synchronous if let Syndrome::Svc(num) = Syndrome::from(esr) => {
            trace!("tf: {:#?}", tf);
            //  handle syscall with fiq enaled
            unsafe {
                aarch64::with_fiq_enabled(|| {
                    handle_syscall(num, tf);
                });
            }
        }
        Kind::Synchronous => {
            unsafe {
                debug!("[MAY BE INVALID] Fault addr: {:x}", FAR_EL1.get());
            }
            panic!("[core-{}] {:#?}, {}, {:#?}", affinity(), info, esr, Syndrome::from(esr));
            // info!("Unhandled exception: {:#?}, {:#?}", info, Syndrome::from(esr));
            // info!("TrapFrame: {:#x?}", tf);
            // let _ = SCHEDULER.kill(tf);
            // GlobalScheduler::switch_to_idle(); // TODO: kill the old proc, set a return code, etc.
        }
        Kind::Irq => {
            let mut handled = false;

            let global_controller = Controller::new();
            if affinity() == 0 {
                // Only core 0 receives global interrupts: currently is only for software based polling that runs on top of Timer3
                for i in Interrupt::iter() {
                    if global_controller.is_pending(i) {
                        unsafe {
                            aarch64::with_fiq_enabled(|| {
                                GLOBAL_IRQ.invoke(i, tf);
                            });
                        }
                        handled = true;
                        break;
                    }
                }
            }

            if !handled {
                let local_controller = LocalController::new(affinity());
                for i in LocalInterrupt::iter() {
                    if local_controller.is_pending(i) {
                        unsafe {
                            aarch64::with_fiq_enabled(|| {
                                local_irq().invoke(i, tf);
                            });
                        }
                        handled = true;
                        break;
                    }
                }
            }

            if !handled {
                panic!("interrupt not handled");
            }
        }
        Kind::Fiq => {
            assert!(affinity() == 0, "FIQ handler called on non-zero CPU");
            // Can only have one FIQ handler: currently is only for USPI
            FIQ.invoke((), tf);
        }
        Kind::SError => {
            debug!("SError trap");
        }
    }

}
