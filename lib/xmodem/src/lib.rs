#![cfg_attr(feature = "no_std", no_std)]
use core::result::Result::Err;
use core::mem::size_of_val;
use core::result::Result::Ok;
use core::assert;
use shim::io;

#[cfg(test)] mod tests;
mod read_ext;
mod progress;

pub use progress::{Progress, ProgressFn};

use read_ext::ReadExt;

const SOH: u8 = 0x01;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;

/// Implementation of the XMODEM protocol.
pub struct Xmodem<R> {
    packet: u8,
    inner: R,
    started: bool,
    progress: ProgressFn,
}

impl Xmodem<()> {
    /// Transmits `data` to the receiver `to` using the XMODEM protocol. If the
    /// length of the total data yielded by `data` is not a multiple of 128
    /// bytes, the data is padded with zeroes and sent to the receiver.
    ///
    /// Returns the number of bytes written to `to`, excluding padding zeroes.
    #[inline]
    pub fn transmit<R, W>(data: R, to: W) -> io::Result<usize>
    where
        W: io::Read + io::Write,
        R: io::Read,
    {
        Xmodem::transmit_with_progress(data, to, progress::noop)
    }

    /// Transmits `data` to the receiver `to` using the XMODEM protocol. If the
    /// length of the total data yielded by `data` is not a multiple of 128
    /// bytes, the data is padded with zeroes and sent to the receiver.
    ///
    /// The function `f` is used as a callback to indicate progress throughout
    /// the transmission. See the [`Progress`] enum for more information.
    ///
    /// Returns the number of bytes written to `to`, excluding padding zeroes.
    pub fn transmit_with_progress<R, W>(mut data: R, to: W, f: ProgressFn) -> io::Result<usize>
    where
        W: io::Read + io::Write,
        R: io::Read,
    {
        let mut transmitter = Xmodem::new_with_progress(to, f);
        let mut packet = [0u8; 128];
        let mut written = 0;
        'next_packet: loop {
            let n = data.read_max(&mut packet)?;
            packet[n..].iter_mut().for_each(|b| *b = 0); // zero out after data is done

            if n == 0 {
                transmitter.write_packet(&[])?;
                return Ok(written);
            }

            for _ in 0..10 {
                match transmitter.write_packet(&packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(_) => {
                        written += n;
                        continue 'next_packet;
                    }
                }
            }

            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "bad transmit"));
        }
    }

    /// Receives `data` from `from` using the XMODEM protocol and writes it into
    /// `into`. Returns the number of bytes read from `from`, a multiple of 128.
    #[inline]
    pub fn receive<R, W>(from: R, into: W) -> io::Result<usize>
    where
        R: io::Read + io::Write,
        W: io::Write,
    {
        Xmodem::receive_with_progress(from, into, progress::noop)
    }

    /// Receives `data` from `from` using the XMODEM protocol and writes it into
    /// `into`. Returns the number of bytes read from `from`, a multiple of 128.
    ///
    /// The function `f` is used as a callback to indicate progress throughout
    /// the reception. See the [`Progress`] enum for more information.
    pub fn receive_with_progress<R, W>(from: R, mut into: W, f: ProgressFn) -> io::Result<usize>
    where
        R: io::Read + io::Write,
        W: io::Write,
    {
        let mut receiver = Xmodem::new_with_progress(from, f);
        let mut packet = [0u8; 128];
        let mut received = 0;
        'next_packet: loop {
            for _ in 0..10 {
                match receiver.read_packet(&mut packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(0) => break 'next_packet,
                    Ok(n) => {
                        received += n;
                        into.write_all(&packet)?;
                        continue 'next_packet;
                    }
                }
            }

            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "bad receive"));
        }

        Ok(received)
    }
}

impl<T: io::Read + io::Write> Xmodem<T> {
    /// Returns a new `Xmodem` instance with the internal reader/writer set to
    /// `inner`. The returned instance can be used for both receiving
    /// (downloading) and sending (uploading).
    pub fn new(inner: T) -> Self {
        Xmodem {
            packet: 1,
            started: false,
            inner,
            progress: progress::noop,
        }
    }

    /// Returns a new `Xmodem` instance with the internal reader/writer set to
    /// `inner`. The returned instance can be used for both receiving
    /// (downloading) and sending (uploading). The function `f` is used as a
    /// callback to indicate progress throughout the transfer. See the
    /// [`Progress`] enum for more information.
    pub fn new_with_progress(inner: T, f: ProgressFn) -> Self {
        Xmodem {
            packet: 1,
            started: false,
            inner,
            progress: f,
        }
    }

    /// Reads a single byte from the inner I/O stream. If `abort_on_can` is
    /// `true`, an error of `ConnectionAborted` is returned if the read byte is
    /// `CAN`.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the inner stream fails or if
    /// `abort_on_can` is `true` and the read byte is `CAN`.
    fn read_byte(&mut self, abort_on_can: bool) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;

        let byte = buf[0];
        if abort_on_can && byte == CAN {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "received CAN",
            ));
        }

        Ok(byte)
    }

    /// Writes a single byte to the inner I/O stream.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the inner stream fails.
    fn write_byte(&mut self, byte: u8) -> io::Result<()> {
        self.inner.write_all(&[byte])
    }

    fn _expect_byte_or_callback(
        &mut self,
        byte: u8,
        expected: &'static str,
        callback: fn(&mut Xmodem<T>, u8) -> io::Result<()>,
    ) -> io::Result<u8> {
        match self.read_byte(false)? {
            read_byte if read_byte == byte => Ok(byte),
            CAN => {
                callback(self, CAN)?;
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, expected))
            }
            bad_byte => {
                callback(self, bad_byte)?;
                Err(io::Error::new(io::ErrorKind::InvalidData, expected))
            }
        }
    }

    fn _cancel_transaction(&mut self) -> io::Result<()> {
        self.write_byte(CAN)
    }

    /// Reads a single byte from the inner I/O stream and compares it to `byte`.
    /// If the bytes match, the byte is returned as an `Ok`. If they differ and
    /// the read byte is not `CAN`, an error of `InvalidData` with the message
    /// `expected` is returned. If they differ and the read byte is `CAN`, an
    /// error of `ConnectionAborted` is returned. In either case, if they bytes
    /// differ, a `CAN` byte is written out to the inner stream.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the inner stream fails, if the read
    /// byte was not `byte`, if the read byte was `CAN` and `byte` is not `CAN`,
    /// or if writing the `CAN` byte failed on byte mismatch.
    fn expect_byte_or_cancel(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        // self._expect_byte_or_callback(byte, expected, |i: &mut Xmodem<T>| -> io::Result<()> { i.write_byte(CAN) })
        self._expect_byte_or_callback(byte, expected, |i: &mut Xmodem<T>, _| -> io::Result<()> {
            i._cancel_transaction()
        })
    }

    /// Reads a single byte from the inner I/O stream and compares it to `byte`.
    /// If they differ, an error of `InvalidData` with the message `expected` is
    /// returned. Otherwise the byte is returned. If `byte` is not `CAN` and the
    /// read byte is `CAN`, a `ConnectionAborted` error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the inner stream fails, or if the read
    /// byte was not `byte`. If the read byte differed and was `CAN`, an error
    /// of `ConnectionAborted` is returned. Otherwise, the error kind is
    /// `InvalidData`.
    fn expect_byte(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        self._expect_byte_or_callback(byte, expected, |_, _| -> io::Result<()> { Ok(()) })
    }

    fn calc_checksum(buf : &[u8]) -> u8 {
        buf.iter().fold(0, |acc: u8, e| acc.wrapping_add(*e))
    }

    /// Reads (downloads) a single packet from the inner stream using the XMODEM
    /// protocol. On success, returns the number of bytes read (always 128).
    ///
    /// The progress callback is called with `Progress::Start` when reception
    /// for the first packet has started and subsequently with
    /// `Progress::Packet` when a packet is received successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or writing to the inner stream fails at any
    /// point. Also returns an error if the XMODEM protocol indicates an error.
    /// In particular, an `InvalidData` error is returned when:
    ///
    ///   * The sender's first byte for a packet isn't `EOT` or `SOH`.
    ///   * The sender doesn't send a second `EOT` after the first.
    ///   * The received packet numbers don't match the expected values.
    ///
    /// An error of kind `Interrupted` is returned if a packet checksum fails.
    ///
    /// An error of kind `ConnectionAborted` is returned if a `CAN` byte is
    /// received when not expected.
    ///
    /// An error of kind `UnexpectedEof` is returned if `buf.len() < 128`.
    pub fn read_packet(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.started {
            self.write_byte(NAK)?;
            self.started = true;
            (self.progress)(Progress::Started);
        }

        if buf.len() < 128 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Buffer length less than 128"))
        }

        match self.read_byte(true)? {
            SOH => {
                self.expect_byte_or_cancel(
                    self.packet,
                    "Read packet sequence expects directly synced packet numbers",
                )?;
                self.expect_byte_or_cancel(
                    255 - self.packet,
                    "Read packet sequence expects one-complement synced packet numbers",
                )?;
                let mut amt_read = 0;
                buf.iter_mut().for_each(|b| {
                    *b = self.read_byte(false).expect("Error reading data");
                    amt_read += size_of_val(b);
                });
                let checksum = Self::calc_checksum(buf);
                self._expect_byte_or_callback(
                    checksum,
                    "Expect ACK after end of packet sequence",
                    |i, _| -> io::Result<()> {
                        i.write_byte(NAK)?;
                        Err(io::Error::new(io::ErrorKind::Interrupted, "expected"))
                    },
                )?;
                self.packet += 1;
                self.write_byte(ACK).map(|_| amt_read)
            }
            EOT => {
                self.write_byte(NAK)?;
                self.expect_byte_or_cancel(
                    EOT,
                    "End of transmission sequence expects second EOT after sending first EOT",
                )?;
                self.write_byte(ACK).map(|_| 0) // beautiful :')
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid byte for a packet",
            )),
        }
    }

    /// Sends (uploads) a single packet to the inner stream using the XMODEM
    /// protocol. If `buf` is empty, end of transmissions is sent. Users of this
    /// interface should ensure that `write_packet(&[])` is called when data
    /// transmission is complete. On success, returns the number of bytes
    /// written.
    ///
    /// The progress callback is called with `Progress::Waiting` before waiting
    /// for the receiver's `NAK`, `Progress::Start` when transmission of the
    /// first packet has started and subsequently with `Progress::Packet` when a
    /// packet is sent successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if reading or writing to the inner stream fails at any
    /// point. Also returns an error if the XMODEM protocol indicates an error.
    /// In particular, an `InvalidData` error is returned when:
    ///
    ///   * The receiver's first byte isn't a `NAK`.
    ///   * The receiver doesn't respond with a `NAK` to the first `EOT`.
    ///   * The receiver doesn't respond with an `ACK` to the second `EOT`.
    ///   * The receiver responds to a complete packet with something besides
    ///     `ACK` or `NAK`.
    ///
    /// An error of kind `UnexpectedEof` is returned if `buf.len() < 128 &&
    /// buf.len() != 0`.
    ///
    /// An error of kind `ConnectionAborted` is returned if a `CAN` byte is
    /// received when not expected.
    ///
    /// An error of kind `Interrupted` is returned if a packet checksum fails.
    pub fn write_packet(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.started {
            assert!(self.packet == 1);
            (self.progress)(Progress::Waiting);
            self.expect_byte_or_cancel(NAK, "Receiver must start the XMODEM protocol with NAK")?;
            self.started = true;
        }

        // If `buf` is empty, end of transmissions is sent
        if buf.is_empty() {
            self.write_byte(EOT)?;
            self.expect_byte(
                NAK,
                "End of transmission sequence expects NAK after sending first EOT",
            )?;
            self.write_byte(EOT)?;
            self.expect_byte(
                ACK,
                "End of transmission sequence expects ACK after sending second EOT",
            )?;
            return Ok(0);
        }

        self.write_byte(SOH)?;
        self.write_byte(self.packet)?;
        self.write_byte(255 - self.packet)?;
        let mut amt_read = 0;
        buf.iter().for_each(|b| {
            self.write_byte(*b).expect("Error writing data");
            amt_read += size_of_val(b);
        });
        let checksum = Self::calc_checksum(buf);
        self.write_byte(checksum)?;

        self._expect_byte_or_callback(
            ACK,
            "Expect ACK after end of packet sequence",
            |_, _| -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::Interrupted, "expected"))
            },
        )?;
        self.packet += 1;
        Ok(amt_read)
    }

    // Not tested
    /// Flush this output stream, ensuring that all intermediately buffered
    /// contents reach their destination.
    ///
    /// # Errors
    ///
    /// It is considered an error if not all bytes could be written due to I/O
    /// errors or EOF being reached.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
