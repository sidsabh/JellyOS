mod frame;
mod syndrome;
mod syscall;

pub mod irq;
use crate::console::kprintln;

pub use self::frame::TrapFrame;

use aarch64::current_el;
use pi::interrupt::{Controller, Interrupt};
use pi::local_interrupt::{LocalController, LocalInterrupt};

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;
use crate::percore;
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
use crate::{shell, IRQ};
/// This function is called when an exception occurs. The `info` parameter
/// specifies the source and kind of exception that has occurred. The `esr` is
/// the value of the exception syndrome register. Finally, `tf` is a pointer to
/// the trap frame for the exception.
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame) {
    match info.kind {
        Kind::Synchronous if let Syndrome::Svc(num) =  Syndrome::from(esr) => {
            // kprintln!("tf: {:#?}", tf);
            handle_syscall(num, tf);
        },
        Kind::Synchronous => {
            // kprintln!("{:#?}, {}, {:#?}", info, esr, Syndrome::from(esr));
            // Preferred Exception Return Address for synchronous
            // is the address of instr that generated exception
            tf.pc += 4;
        }
        Kind::Irq => {
            let controller = Controller::new();
            for i in Interrupt::iter() {
                if controller.is_pending(*i) {
                    // kprintln!("{:#?}, idx:{:#?} ", info, Interrupt::to_index(*i));
                    IRQ.invoke(*i, tf);
                    break;
                }
            }
        }
        Kind::Fiq => {},
        Kind::SError => {},
    }
}
