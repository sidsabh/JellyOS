#![allow(non_snake_case)]

use alloc::boxed::Box;
use alloc::string::String;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::slice;
use core::time::Duration;

use pi::interrupt::{Controller, Interrupt};
use pi::timer::spin_sleep;
use smoltcp::wire::EthernetAddress;

use crate::mutex::Mutex;
use crate::net::Frame;
use crate::traps::irq::IrqHandlerRegistry;
use crate::{ALLOCATOR, FIQ};

const DEBUG_USPI: bool = true; // define as false when not networking
pub macro uspi_trace {
    () => (if DEBUG_USPI { trace!("\n") } ),
    ($fmt:expr) => (if DEBUG_USPI { trace!(concat!($fmt, "\n")) }),
    ($fmt:expr, $($arg:tt)*) => (if DEBUG_USPI { trace!(concat!($fmt, "\n"), $($arg)*) })
}

pub type TKernelTimerHandle = u64;
pub type TKernelTimerHandler = Option<
    unsafe extern "C" fn(hTimer: TKernelTimerHandle, pParam: *mut c_void, pContext: *mut c_void),
>;
pub type TInterruptHandler = Option<unsafe extern "C" fn(pParam: *mut c_void)>;

mod inner {
    use core::convert::TryInto;
    use core::ptr;
    use core::time::Duration;

    use super::{TKernelTimerHandle, TKernelTimerHandler};
    use crate::net::Frame;
    use crate::param::USPI_TIMER_HZ;

    #[allow(non_camel_case_types)]
    type c_uint = usize;

    pub struct USPi(());

    extern "C" {
        /// Returns 0 on failure
        fn USPiInitialize() -> i32;
        /// Check if the ethernet controller is available.
        /// Returns != 0 if available
        fn USPiEthernetAvailable() -> i32;
        fn USPiGetMACAddress(Buffer: &mut [u8; 6]);
        /// Returns != 0 if link is up
        fn USPiEthernetIsLinkUp() -> i32;
        /// Returns 0 on failure
        fn USPiSendFrame(pBuffer: *const u8, nLength: u32) -> i32;
        /// pBuffer must have size USPI_FRAME_BUFFER_SIZE
        /// Returns 0 if no frame is available or on failure
        fn USPiReceiveFrame(pBuffer: *mut u8, pResultLength: *mut u32) -> i32;
        /// Returns a timer handle (0 on failure)
        fn TimerStartKernelTimer(
            pThis: TKernelTimerHandle,
            nDelay: c_uint, // in HZ units
            pHandler: TKernelTimerHandler,
            pParam: *mut core::ffi::c_void,
            pContext: *mut core::ffi::c_void,
        ) -> c_uint;
        fn TimerGet() -> TKernelTimerHandle;
    }

    impl !Sync for USPi {}

    impl USPi {
        /// The caller should assure that this function is called only once
        /// during the lifetime of the kernel.
        pub unsafe fn initialize() -> Self {
            assert!(USPiInitialize() != 0);
            USPi(())
        }

        /// Returns whether ethernet is available on RPi
        pub fn is_eth_available(&mut self) -> bool {
            unsafe { USPiEthernetAvailable() != 0 }
        }

        /// Returns MAC address of RPi
        pub fn get_mac_address(&mut self, buf: &mut [u8; 6]) {
            unsafe { USPiGetMACAddress(buf) }
        }

        /// Checks whether RPi ethernet link is up or not
        pub fn is_eth_link_up(&mut self) -> bool {
            unsafe { USPiEthernetIsLinkUp() != 0 }
        }

        /// Sends an ethernet frame using USPiSendFrame
        pub fn send_frame(&mut self, frame: &Frame) -> Option<i32> {
            trace!("Send frame {:?}", frame);
            let result = unsafe { USPiSendFrame(frame.as_ptr(), frame.len()) };
            match result {
                0 => None,
                n => Some(n),
            }
        }

        /// Receives an ethernet frame using USPiRecvFrame
        pub fn recv_frame<'a>(&mut self, frame: &mut Frame) -> Option<i32> {
            let mut result_len = 0;
            trace!("Recv frame {:?}", frame);
            let result = unsafe { USPiReceiveFrame(frame.as_mut_ptr(), &mut result_len) };
            frame.set_len(result_len);
            match result {
                0 => None,
                n => Some(n),
            }
        }

        /// A wrapper function to `TimerStartKernelHandler`.
        pub fn start_kernel_timer(&mut self, delay: Duration, handler: TKernelTimerHandler) {
            trace!(
                "Core {}, delay {:?}, handler {:?}",
                aarch64::affinity(),
                &delay,
                handler.map(|v| v as usize as *mut u8)
            );

            let divisor = (1000 / USPI_TIMER_HZ) as u128;
            let delay_as_hz = (delay.as_millis() + divisor - 1) / divisor;

            if let Ok(c_delay) = delay_as_hz.try_into() {
                unsafe {
                    TimerStartKernelTimer(
                        TimerGet(),
                        c_delay,
                        handler,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                }
            }
        }
    }
}

pub use inner::USPi;

unsafe fn layout(size: usize) -> Layout {
    Layout::from_size_align_unchecked(size + core::mem::size_of::<usize>(), 16)
}


#[no_mangle]
fn malloc(size: u32) -> *mut c_void {
    let size = size as usize;
    let layout = unsafe {layout(size)};
    let ptr = unsafe { ALLOCATOR.alloc(layout) };

    if ptr.is_null() {
        uspi_trace!("malloc failed");
        return ptr as *mut c_void;
    }
    let ptr = unsafe { (ptr as *mut usize).offset(1) };
    unsafe { *ptr.offset(-1) = size as usize };
    uspi_trace!("malloc: size = {}, ptr = {:?}", size, ptr);
    return ptr as *mut c_void;

}

#[no_mangle]
fn free(ptr: *mut c_void) {
    let ptr = unsafe { (ptr as *mut usize).offset(-1) };
    let size = unsafe { *ptr };
    let layout = unsafe { layout(size) };
    unsafe { ALLOCATOR.dealloc(ptr as *mut u8, layout) };
    uspi_trace!("free: size = {}, ptr = {:?}", size, ptr);
}

#[no_mangle]
pub fn TimerSimpleMsDelay(nMilliSeconds: u32) {
    let delay = Duration::from_millis(nMilliSeconds as u64);
    uspi_trace!("TimerSimpleMsDelay: {:?}", delay);
    spin_sleep(delay);
}

#[no_mangle]
pub fn TimerSimpleusDelay(nMicroSeconds: u32) {
    let delay = Duration::from_micros(nMicroSeconds as u64);
    uspi_trace!("TimerSimpleusDelay: {:?}", delay);
    spin_sleep(delay);
}

#[no_mangle]
pub fn MsDelay(nMilliSeconds: u32) {
    uspi_trace!("MsDelay: {:?}", nMilliSeconds);
    // TODO: fix
    TimerSimpleMsDelay(nMilliSeconds);
}

#[no_mangle]
pub fn usDelay(nMicroSeconds: u32) {
    uspi_trace!("usDelay: {:?}", nMicroSeconds);
    // TODO: fix
    TimerSimpleusDelay(nMicroSeconds);
}

/// Registers `pHandler` to the kernel's IRQ handler registry.
/// When the next time the kernel receives `nIRQ` signal, `pHandler` handler
/// function should be invoked with `pParam`.
///
/// If `nIRQ == Interrupt::Usb`, register the handler to FIQ interrupt handler
/// registry. Otherwise, register the handler to the global IRQ interrupt handler.
#[no_mangle]
pub unsafe fn ConnectInterrupt(nIRQ: u32, pHandler: TInterruptHandler, pParam: *mut c_void) {
    // kern/src/net/uspi.rs. First, assert that nIRQ is one of Interrupt::Usb or Interrupt::Timer3. Then, if the request is for USB interrupt, enable FIQ handling of USB interrupt and register a handler that invokes provided pHandler to the FIQ handler registry (crate::FIQ). Otherwise, enable IRQ handling of Timer3 interrupt and register a handler that invokes provided pHandler to the global IRQ handler registry (crate::GLOBAL_IRQ).
    assert!(nIRQ == Interrupt::Usb as u32 || nIRQ == Interrupt::Timer3 as u32);
    if nIRQ == Interrupt::Usb as u32 {
       // Enable FIQ handling of USB interrupt
    } else {
        // Enable IRQ handling of Timer3 interrupt
    }
    uspi_trace!(
        "ConnectInterrupt: nIRQ = {}, pHandler = {:?}, pParam = {:?}",
        nIRQ,
        pHandler,
        pParam
    );
}

/// Writes a log message from USPi using `uspi_trace!` macro.
#[no_mangle]
pub unsafe fn DoLogWrite(_pSource: *const u8, _Severity: u32, pMessage: *const u8) {
    uspi_trace!(
        "USPi: {}",
        String::from_utf8_lossy(slice::from_raw_parts(pMessage, 256))
    );
}

#[no_mangle]
pub fn DebugHexdump(_pBuffer: *const c_void, _nBufLen: u32, _pSource: *const u8) {
    unimplemented!("You don't have to implement this")
}

#[no_mangle]
pub unsafe fn uspi_assertion_failed(pExpr: *const u8, pFile: *const u8, nLine: u32) {
    uspi_trace!(
        "Assertion failed: {} in file {} at line {}",
        String::from_utf8_lossy(slice::from_raw_parts(pExpr, 256)),
        String::from_utf8_lossy(slice::from_raw_parts(pFile, 256)),
        nLine
    );
}

pub struct Usb(pub Mutex<Option<USPi>>);

impl Usb {
    pub const fn uninitialized() -> Usb {
        Usb(Mutex::new(None))
    }

    pub fn initialize(&self) {
        let mut inner = self.0.lock();
        if let None = *inner {
            *inner = Some(unsafe { USPi::initialize() });
        }
    }

    pub fn is_eth_available(&self) -> bool {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .is_eth_available()
    }

    pub fn get_eth_addr(&self) -> EthernetAddress {
        let mut buf = [0; 6];
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .get_mac_address(&mut buf);
        return EthernetAddress::from_bytes(&buf);
    }

    pub fn is_eth_link_up(&self) -> bool {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .is_eth_link_up()
    }

    pub fn send_frame(&self, frame: &Frame) -> Option<i32> {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .send_frame(frame)
    }

    pub fn recv_frame(&self, frame: &mut Frame) -> Option<i32> {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .recv_frame(frame)
    }

    pub fn start_kernel_timer(&self, delay: Duration, handler: TKernelTimerHandler) {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .start_kernel_timer(delay, handler)
    }
}
