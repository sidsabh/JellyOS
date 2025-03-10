use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[repr(align(32))]
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    lock: AtomicBool,
    owner: AtomicUsize,
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T: 'a> {
    lock: &'a Mutex<T>,
}

impl<'a, T> !Send for MutexGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for MutexGuard<'a, T> {}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Mutex<T> {
        Mutex {
            lock: AtomicBool::new(false),
            owner: AtomicUsize::new(usize::max_value()),
            data: UnsafeCell::new(val),
        }
    }
}

use crate::percore::*;
impl<T> Mutex<T> {
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if is_mmu_ready() {
            if !self
                .lock
                .swap(true, Ordering::Acquire)
            {
                self.owner.store(getcpu(), Ordering::Relaxed);
                Some(MutexGuard { lock: &self })
            } else {
                None
            }
        } else {
            let this = 0;
            assert!(aarch64::affinity() == this);
            if !self.lock.load(Ordering::Relaxed) || self.owner.load(Ordering::Relaxed) == this {
                self.lock.store(true, Ordering::Relaxed);
                self.owner.store(this, Ordering::Relaxed);
                Some(MutexGuard { lock: &self })
            } else {
                None
            }
        }
    }

    #[inline(never)]
    pub fn lock(&self) -> MutexGuard<T> {
        // Wait until we can "aquire" the lock, then "acquire" it.
        loop {
            match self.try_lock() {
                Some(guard) => return guard,
                None => continue,
            }
        }
    }

    fn unlock(&self) {
        if is_mmu_ready() {
            putcpu(self.owner.load(Ordering::Relaxed));
            self.lock.store(false, Ordering::Release);
        } else {
            self.lock.store(false, Ordering::Relaxed);
        }
    }
}

impl<'a, T: 'a> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: 'a> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: 'a> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock()
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            None => f.debug_struct("Mutex").field("data", &"<locked>").finish(),
        }
    }
}
