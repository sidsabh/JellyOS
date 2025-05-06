#![no_std]

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::percore::{getcpu, putcpu, is_mmu_ready};   // ←—— bring the helpers in

/* ───────────────────────── Mutex ───────────────────────────────────────── */

#[repr(align(32))]
pub struct Mutex<T> {
    lock:  AtomicBool,     // false = unlocked
    owner: AtomicUsize,    // CPU id that holds the lock (§ owner == usize::MAX ⇒ free)
    data:  UnsafeCell<T>,
}

unsafe impl<T: Send> Send  for Mutex<T> {}
unsafe impl<T: Send> Sync  for Mutex<T> {}

pub struct MutexGuard<'a, T: 'a> {
    lock: &'a Mutex<T>,
}

impl<'a, T> !Send for MutexGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for MutexGuard<'a, T> {}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Self {
        Self {
            lock:  AtomicBool::new(false),
            owner: AtomicUsize::new(usize::MAX),
            data:  UnsafeCell::new(val),
        }
    }

    /// Try to grab the lock without blocking.
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if is_mmu_ready() {
            /* --------- multicore / normal runtime path --------- */
            if self
                .lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                /* increment pre‑emption depth for this CPU */
                let cpu = getcpu();
                self.owner.store(cpu, Ordering::Relaxed);
                Some(MutexGuard { lock: self })
            } else {
                None
            }
        } else {
            /* --------- very‑early boot: single core only -------- */
            debug_assert_eq!(aarch64::affinity(), 0);
            let held_by_me = self.owner.load(Ordering::Relaxed) == 0;

            if !self.lock.load(Ordering::Relaxed) || held_by_me {
                self.lock.store(true, Ordering::Relaxed);
                self.owner.store(0, Ordering::Relaxed);
                Some(MutexGuard { lock: self })
            } else {
                None
            }
        }
    }

    /// Spin until the mutex becomes available.
    pub fn lock(&self) -> MutexGuard<T> {
        loop {
            if let Some(g) = self.try_lock() {
                return g;
            }
            /* polite spin */
            #[cfg(target_arch = "aarch64")]
            unsafe { core::arch::asm!("wfe", "yield", options(nomem, nostack)) };

            #[cfg(not(target_arch = "aarch64"))]
            core::hint::spin_loop();
        }
    }

    /// Internal: release the lock.
    fn unlock(&self) {
        if is_mmu_ready() {
            /* decrement the counter for whichever CPU owned the lock */
            let cpu = self.owner.load(Ordering::Relaxed);
            putcpu(cpu);
        }

        /* clear owner first, then release the boolean */
        self.owner.store(usize::MAX, Ordering::Relaxed);
        self.lock.store(false, Ordering::Release);

        /* wake other waiters */
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("sev", options(nomem, nostack)); }
    }
}

/* ───────────────────── Guard impls ─────────────────────────────────────── */

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/* ───────────────────── Debug impl ──────────────────────────────────────── */

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(g) => f.debug_struct("Mutex").field("data", &&*g).finish(),
            None    => f.debug_struct("Mutex").field("data", &"<locked>").finish(),
        }
    }
}
