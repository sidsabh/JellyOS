use core::fmt;
use core::time::Duration;

use shim::const_assert_size;
use shim::io;

use volatile::prelude::*;
use volatile::{ReadVolatile, Reserved, Volatile};

use crate::common::{CLOCK_HZ, IO_BASE};
use crate::gpio::{Function, Gpio};
use crate::timer;

/// The base address for the `MU` registers.
const MU_REG_BASE: usize = IO_BASE + 0x215040;

/// The `AUXENB` register from page 9 of the BCM2837 documentation.
const AUX_ENABLES: *mut Volatile<u8> = (IO_BASE + 0x215004) as *mut Volatile<u8>;

/// Enum representing bit fields of the `AUX_MU_LSR_REG` register.
#[repr(u8)]
enum LsrStatus {
    DataReady = 1,
    TxAvailable = 1 << 5,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    IO: Volatile<u8>,        // Mini Uart I/O Data
    __r0: [Reserved<u8>; 3],
    IER: Volatile<u8>,       // Mini Uart Interrupt Enable
    __r1: [Reserved<u8>; 3],
    IIR: Volatile<u8>,       // Mini Uart Interrupt Identify
    __r2: [Reserved<u8>; 3],
    LCR: Volatile<u8>,       // Mini Uart Line Control
    __r3: [Reserved<u8>; 3],
    MCR: Volatile<u8>,       // Mini Uart Modem Control
    __r4: [Reserved<u8>; 3],
    LSR: ReadVolatile<u8>,   // Mini Uart Line Status
    __r5: [Reserved<u8>; 3],
    MSR: ReadVolatile<u8>,   // Mini Uart Modem Status
    __r6: [Reserved<u8>; 3],
    SCR: Volatile<u8>,       // Mini Uart Scratch
    __r7: [Reserved<u8>; 3],
    CNTL: Volatile<u8>,      // Mini Uart Extra Control
    __r8: [Reserved<u8>; 3],
    STAT: ReadVolatile<u32>, // Mini Uart Extra Status
    BAUD: Volatile<u16>,     // Mini Uart Baudrate
    __r9: [Reserved<u8>; 2]
}

const_assert_size!(Registers, 0x7E21506C - 0x7E215040);

/// The Raspberry Pi's "mini UART".
pub struct MiniUart {
    registers: &'static mut Registers,
    timeout: Option<Duration>,
}

impl MiniUart {
    /// Initializes the mini UART by enabling it as an auxiliary peripheral,
    /// setting the data size to 8 bits, setting the BAUD rate to ~115200 (baud
    /// divider of 270), setting GPIO pins 14 and 15 to alternative function 5
    /// (TXD1/RDXD1), and finally enabling the UART transmitter and receiver.
    ///
    /// By default, reads will never time out. To set a read timeout, use
    /// `set_read_timeout()`.
    pub fn new() -> MiniUart {
        let registers = unsafe {
            // Enable the mini UART as an auxiliary device.
            (*AUX_ENABLES).or_mask(1);
            &mut *(MU_REG_BASE as *mut Registers)
        };

        registers.LCR.write(0b11); // 8 bit mode
        let baudrate_reg = (CLOCK_HZ / (8 * 115200)) as u16; // baudrate
        registers.BAUD.write(baudrate_reg - 1);

        // setting GPIO pins as alternative function 5
        Gpio::new(14).into_alt(Function::Alt5);
        Gpio::new(15).into_alt(Function::Alt5);
        
        registers.CNTL.or_mask(0b11); // enable UART as transmitter, reciever
        MiniUart {
            registers,
            timeout: None,
        }
    }

    /// Set the read timeout to `t` duration.
    pub fn set_read_timeout(&mut self, t: Duration) {
        self.timeout = Some(t);
    }

    /// Write the byte `byte`. This method blocks until there is space available
    /// in the output FIFO.
    pub fn write_byte(&mut self, byte: u8) {
        while !self.registers.LSR.has_mask(LsrStatus::TxAvailable as u8) {}
        self.registers.IO.write(byte);
    }

    /// Returns `true` if there is at least one byte ready to be read. If this
    /// method returns `true`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately. This method does not block.
    pub fn has_byte(&self) -> bool {
        self.registers.LSR.has_mask(LsrStatus::DataReady as u8)
    }

    /// Blocks until there is a byte ready to read. If a read timeout is set,
    /// this method blocks for at most that amount of time. Otherwise, this
    /// method blocks indefinitely until there is a byte to read.
    ///
    /// Returns `Ok(())` if a byte is ready to read. Returns `Err(())` if the
    /// timeout expired while waiting for a byte to be ready. If this method
    /// returns `Ok(())`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately.
    pub fn wait_for_byte(&self) -> Result<(), ()> {
        match self.timeout {
            Some(timeout) => {
                let ct = timer::current_time();
                while !self.has_byte() && timer::current_time() < ct + timeout {}

                if self.has_byte() {
                    Ok(())
                } else {
                    Err(())
                }
            }
            None => {
                while !self.has_byte() {}
                Ok(())
            }
        }
    }

    /// Reads a byte. Blocks indefinitely until a byte is ready to be read.
    pub fn read_byte(&mut self) -> u8 {
        self.wait_for_byte().expect("Read failed: timeout");
        self.registers.IO.read()
    }
}

// A b'\r' byte should be written
// before writing any b'\n' byte.
use core::fmt::Error;
impl fmt::Write for MiniUart {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        s.as_bytes().iter().for_each(|b| {
            if *b == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(*b)
        });
        Ok(())
    }
}

mod uart_io {
    use super::io;
    use super::MiniUart;
    use shim::io::{Read, Write};
    use volatile::prelude::*;

    // The `io::Read::read()` implementation must respect the read timeout by
    // waiting at most that time for the _first byte_. It should not wait for
    // any additional bytes but _should_ read as many bytes as possible. If the
    // read times out, an error of kind `TimedOut` should be returned.
    //
    // The `io::Write::write()` method must write all of the requested bytes
    // before returning.
    impl Read for MiniUart {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut ctr: usize = 0;
            for byte in buf.iter_mut() {
                match self.wait_for_byte() {
                    Ok(_) => {
                        *byte = self.read_byte();
                        ctr += 1;
                    }
                    Err(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Timed out waiting to read byte",
                        ))
                    }
                }
                self.timeout = None;
            }
            Ok(ctr)
        }
    }

    impl Write for MiniUart {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut ctr: usize = 0;
            buf.iter().for_each(|b| {
                if *b == b'\n' {
                    self.write_byte(b'\r');
                }
                self.write_byte(*b);
                ctr += 1;
            }
            );
            Ok(ctr)
        }
        fn flush(&mut self) -> io::Result<()> {
            while !self.registers.LSR.has_mask(1 << 6) {}
            Ok(())
        }
    }
}
