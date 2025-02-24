mod process;
mod scheduler;
mod stack;
mod state;

pub use self::process::{Id, Process};
pub use self::scheduler::GlobalScheduler;
pub use self::stack::Stack;
pub use self::state::State;
use fat32::vfat::VFatHandle;

use shim::io::{Read, Write};
use shim::io;

/// Console file, used for stdin, stdout, stderr.
#[derive(Debug)]
pub struct ConsoleFile;

impl ProcessFileT for ConsoleFile {
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }
    fn size(&self) -> Option<usize> { None } // No fixed size for console

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut console = crate::console::CONSOLE.lock();
        console.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut console = crate::console::CONSOLE.lock();
        console.write(buf)
    }
    
    fn seek(&mut self, pos: usize) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "Cannot seek on console"))
    }
}

// Offset maintained internally
impl<T: VFatHandle> ProcessFileT for fat32::vfat::File<T> {
    fn is_dir(&self) -> bool {
        false
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { true }

    fn size(&self) -> Option<usize> {
        Some(self.metadata.size as usize)
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read: usize = io::Read::read(self, buf)?;
        Ok(bytes_read)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = io::Write::write(self, buf)?;
        Ok(bytes_written)
    }

    fn seek(&mut self, pos: usize) -> io::Result<()> {
        io::Seek::seek(self, io::SeekFrom::Start(pos as u64))?;
        Ok(())
    }
}



use fat32::traits::Dir;
use alloc::string::String;
use fat32::traits::Entry;
impl<T: VFatHandle> ProcessFileT for fat32::vfat::Dir<T> {
    fn is_dir(&self) -> bool {
        true
    }
    fn is_readable(&self) -> bool { true }
    fn is_writable(&self) -> bool { false }

    fn size(&self) -> Option<usize> {
        None
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "Cannot read a directory"))
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "Cannot write to a directory"))
    }

    fn seek(&mut self, pos: usize) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "Cannot seek on a directory"))
    }

    fn readdir(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut entries = String::new();
        for entry in self.entries()? {
            entries.push_str(entry.name());
            entries.push('\n');
        }
        let bytes = entries.as_bytes();
        let len = buf.len().min(bytes.len());
        buf[..len].copy_from_slice(&bytes[..len]);
        Ok(len)
    }
}

pub trait ProcessFileT: Send + Sync + core::fmt::Debug {
    fn is_dir(&self) -> bool { false }
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
    fn size(&self) -> Option<usize>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn seek(&mut self, pos: usize) -> io::Result<()>;
    fn readdir(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a directory"))
    }

}



use alloc::sync::Arc;
use crate::mutex::Mutex;
use alloc::boxed::Box;
#[derive(Debug, Clone)]
pub struct ProcessFile {
    pub handle: Arc<Mutex<Box<dyn ProcessFileT>>>, // Shared, mutable file descriptor
    pub offset: usize,
}


impl Clone for ConsoleFile {
    fn clone(&self) -> Self {
        ConsoleFile
    }
}


#[derive(Clone)]
pub struct ChildFuture {
    pub done: Option<Arc<Mutex<bool>>>, // Shared flag between parent & child
}

impl ChildFuture {
    /// Create a new ChildFuture (Initially not done)
    pub fn new() -> Self {
        Self {
            done: Some(Arc::new(Mutex::new(false))),
        }
    }

    /// Mark the child process as done
    pub fn complete(&self) {
        if let Some(done) = &self.done {
            let mut done = done.lock();
            *done = true;
        }
    }

    pub fn is_done(&self) -> bool {
        if let Some(done) = &self.done {
            let done = done.lock();
            *done
        } else {
            true
        }
    }

    /// Blocks until the child is done
    pub fn wait(&self) {
        if let Some(done) = &self.done {
            let done = done.lock();
            while !*done {
                core::hint::spin_loop(); // Efficient waiting (reduces CPU usage)
            }
        }
    }
}

impl core::fmt::Debug for ChildFuture {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ChildFuture {{ done: {:?} }}", *self.done.as_ref().unwrap().lock())
    }
}