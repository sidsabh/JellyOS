#![cfg_attr(not(test), no_std)]
#![feature(decl_macro)]

mod linked_list;
mod util;
pub use self::util::{align_down, align_up};

mod bin;
mod bump;

type AllocatorImpl = bin::Allocator;

#[cfg(test)]
mod tests;

use core::alloc::{GlobalAlloc, Layout};
use core::fmt;


use spin::Mutex;

/// `LocalAlloc` is an analogous trait to the standard library's `GlobalAlloc`,
/// but it takes `&mut self` in `alloc()` and `dealloc()`.
pub trait LocalAlloc {
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}

/// Thread-safe (locking) wrapper around a particular memory allocator.
pub struct Allocator(Mutex<Option<AllocatorImpl>>);

impl Allocator {
    /// Returns an uninitialized `Allocator`.
    ///
    /// The allocator must be initialized by calling `initialize()` before the
    /// first memory allocation. Failure to do will result in panics.
    pub const fn uninitialized() -> Self {
        Allocator(Mutex::new(None))
    }

    /// Initializes the memory allocator.
    /// The caller should assure that the method is invoked only once during the
    /// kernel initialization.
    ///
    /// # Panics
    ///
    /// Panics if the system's memory map could not be retrieved.
    pub unsafe fn initialize(&self, start: usize, end: usize) {
        *self.0.lock() = Some(AllocatorImpl::new(start, end));
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // kprintln!("allocing: {}  bytes", layout.size());
        let aligned_layout = Layout::from_size_align(layout.size(), layout.align().max(4))
            .expect("Invalid layout for allocation");
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .alloc(aligned_layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // kprintln!("deallocing: {}  bytes", layout.size());
        let aligned_layout = Layout::from_size_align(layout.size(), layout.align().max(4))
            .expect("Invalid layout for deallocation");
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .dealloc(ptr, aligned_layout);
    }
}


impl fmt::Debug for Allocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.lock().as_mut() {
            Some(ref alloc) => {
                write!(f, "{:?}", alloc)?;
            }
            None => write!(f, "Not yet initialized")?,
        }
        Ok(())
    }
}
