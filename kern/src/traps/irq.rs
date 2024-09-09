use alloc::boxed::Box;
use core::borrow::Borrow;
use core::ops::Index;

use pi::interrupt::Interrupt;
use pi::local_interrupt::LocalInterrupt;

use crate::mutex::Mutex;
use crate::traps::TrapFrame;

// Programmer Guide Chapter 10
// AArch64 Exception Handling
pub type IrqHandler = Box<dyn FnMut(&mut TrapFrame) + Send>;
type IrqHandlerMutex = Mutex<Option<IrqHandler>>;

type GlobalIrqHandlers = [IrqHandlerMutex; Interrupt::MAX];
type LocalIrqHandlers = [IrqHandlerMutex; LocalInterrupt::MAX];

/// Global IRQ handler registry.
pub struct GlobalIrq(GlobalIrqHandlers);
/// Local (per-core) IRQ handler registry. (QA7: Chapter 4)
pub struct LocalIrq(LocalIrqHandlers);
/// Global FIQ handler registry. Our kerenl supports only one FIQ interrupt.
pub struct Fiq(IrqHandlerMutex);

impl GlobalIrq {
    pub const fn new() -> GlobalIrq {
        GlobalIrq([
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
        ])
    }
}

impl LocalIrq {
    pub const fn new() -> LocalIrq {
        LocalIrq([
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
        ])
    }
}

impl Fiq {
    pub const fn new() -> Fiq {
        Fiq(Mutex::new(None))
    }
}

impl Index<Interrupt> for GlobalIrq {
    type Output = IrqHandlerMutex;

    fn index(&self, int: Interrupt) -> &IrqHandlerMutex {
        use Interrupt::*;
        let index = match int {
            Timer1 => 0,
            Timer3 => 1,
            Usb => 2,
            Gpio0 => 3,
            Gpio1 => 4,
            Gpio2 => 5,
            Gpio3 => 6,
            Uart => 7,
        };
        &self.0[index]
    }
}

impl Index<LocalInterrupt> for LocalIrq {
    type Output = IrqHandlerMutex;

    fn index(&self, int: LocalInterrupt) -> &IrqHandlerMutex {
        // Lab 5 1.C
        use LocalInterrupt::*;
        let index = match int {
            CntPsIrq => 0,         // CNTPSIRQ interrupt (Physical Timer -1)
            CntPnsIrq => 1,        // CNTPNSIRQ interrupt
            CntHpIrq => 2,         // CNTHPIRQ interrupt
            CntvIrq => 3,          // CNTVIRQ interrupt
            Mailbox0 => 4,         // Mailbox 0 interrupt
            Mailbox1 => 5,         // Mailbox 1 interrupt
            Mailbox2 => 6,         // Mailbox 2 interrupt
            Mailbox3 => 7,         // Mailbox 3 interrupt
            GpuInterrupt => 8,     // GPU interrupt <Can be high in one core only>
            PmuInterrupt => 9,     // PMU interrupt
            AxiOutstanding => 10,  // AXI-outstanding interrupt <For core 0 only!> all others are 0
            LocalTimer => 11,      // Local timer interrupt
        };
        &self.0[index]
    }
}

impl Index<()> for Fiq {
    type Output = IrqHandlerMutex;

    fn index(&self, _: ()) -> &IrqHandlerMutex {
        // Lab 5 2.B
        unimplemented!("FIQ Index")
    }
}

/// A trait that defines the behavior of an IRQ (and FIQ) handler registry.
pub trait IrqHandlerRegistry<I> {
    fn register(&self, int: I, handler: IrqHandler);
    fn invoke(&self, int: I, tf: &mut TrapFrame);
}

/// A blanket implementation of `IrqHandlerRegistry` trait for all indexable
/// struct that returns `IrqHandlerMutex`.
impl<I, T> IrqHandlerRegistry<I> for T
where
    T: Index<I, Output = IrqHandlerMutex>,
{
    /// Register an irq handler for an interrupt.
    fn register(&self, int: I, handler: IrqHandler) {
	self.index(int).lock().replace(handler);
    }

    /// Executes an irq handler for the given interrupt.
    fn invoke(&self, int: I, tf: &mut TrapFrame) {
         if let Some(handler) = self.index(int).lock().as_mut() {
	    handler(tf);
        }
    }
}
