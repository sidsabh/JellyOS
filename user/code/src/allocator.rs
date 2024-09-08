use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
const PAGE_SIZE : usize = 16 * 1024;
const USER_IMG_BASE : usize = 0xffff_ffff_c000_0000;

struct InnerAlloc(UnsafeCell<(usize, usize)>);

use core::marker::{Send, Sync};
unsafe impl Send for InnerAlloc {}

unsafe impl Sync for InnerAlloc {}

pub struct GlobalAllocator(InnerAlloc);

extern "C" {
    static __text_end: u8;
}
//TODO: uninitialize then
impl GlobalAllocator {
    const fn new() -> Self {
        unsafe {
            return GlobalAllocator(InnerAlloc(UnsafeCell::new((USER_IMG_BASE+PAGE_SIZE*2, USER_IMG_BASE+PAGE_SIZE*4)))) 
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let (beg, end) = &mut *self.0.0.get();

            if *beg & (layout.align() - 1) != 0 {
                *beg = *beg & (!(layout.align() - 1)) + layout.align();
            }

            let location = *beg as *mut u8;
            *beg += layout.size();

            location
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
#[global_allocator]
pub static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();


pub unsafe fn get_data() -> usize {
    let ga = &ALLOCATOR;
    let (beg, end) = &mut *ga.0.0.get();
    return *beg;
}

