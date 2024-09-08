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


pub trait MutexTrait<T> {
    fn lock(&self) -> &mut T;
}
/// `LocalAlloc` is an analogous trait to the standard library's `GlobalAlloc`,
/// but it takes `&mut self` in `alloc()` and `dealloc()`.
pub trait LocalAlloc {
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}

/// Thread-safe (locking) wrapper around a particular memory allocator.

pub struct Allocator<M: MutexTrait<Option<AllocatorImpl>>> {
    inner: M,
}

impl<M: MutexTrait<Option<AllocatorImpl>>> Allocator<M> {
    /// Returns an uninitialized `Allocator`.
    ///
    /// The allocator must be initialized by calling `initialize()` before the
    /// first memory allocation. Failure to do so will result in panics.
    pub const fn uninitialized(mutex: M) -> Self {
        Allocator {
            inner: mutex,
        }
    }

    /// Initializes the memory allocator.
    /// The caller should assure that the method is invoked only once during the
    /// kernel initialization.
    ///
    /// # Panics
    ///
    /// Panics if the system's memory map could not be retrieved.
    pub unsafe fn initialize(&self, start: usize, end: usize) {
        *self.inner.lock() = Some(AllocatorImpl::new(start, end));
    }
}

unsafe impl<M: MutexTrait<Option<AllocatorImpl>>> GlobalAlloc for Allocator<M> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let aligned_layout = Layout::from_size_align(layout.size(), layout.align().max(4))
            .expect("Invalid layout for allocation");
        self.inner
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .alloc(aligned_layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let aligned_layout = Layout::from_size_align(layout.size(), layout.align().max(4))
            .expect("Invalid layout for deallocation");
        self.inner
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .dealloc(ptr, aligned_layout);
    }
}

impl<M: MutexTrait<Option<AllocatorImpl>>> fmt::Debug for Allocator<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.lock().as_mut() {
            Some(ref alloc) => {
                write!(f, "{:?}", alloc)?;
            }
            None => write!(f, "Not yet initialized")?,
        }
        Ok(())
    }
}

