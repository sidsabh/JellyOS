extern crate heap;
use heap::Allocator;
use heap::align_up;

use spin::Mutex;
use heap::MutexTrait;


impl<T> MutexTrait<T> for Mutex<T> {
    fn lock(&self) -> &mut T {
        self.lock()
    }
}

#[global_allocator]
pub static ALLOCATOR: Allocator<Mutex> = Allocator::uninitialized(Mutex::new(None));

extern "C" {
    static __text_end: u8;
}
const PAGE_SIZE : usize = 64 * 1024;
const USER_IMG_BASE : usize = 0xffff_ffff_c000_0000;
const USER_HEAP_PAGES : usize = 2;


/// Returns the (start address, end address) of the available memory on this
/// system if it can be determined. If it cannot, `None` is returned.
///
/// This function is expected to return `Some` under all normal cirumstances.
pub fn memory_map() -> (usize, usize) {
    let mut binary_end = unsafe { (&__text_end as *const u8) as usize };
    binary_end = align_up(binary_end, PAGE_SIZE);
    (binary_end, binary_end + USER_HEAP_PAGES*PAGE_SIZE - 1)
}

