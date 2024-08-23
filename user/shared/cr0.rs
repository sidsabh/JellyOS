use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::mem;
use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::write_volatile;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

const PAGE_SIZE : usize = 16 * 1024;
const USER_IMG_BASE : usize = 0xffff_ffff_c000_0000;
const PAGE_ONE_END : usize = PAGE_SIZE + USER_IMG_BASE;

unsafe fn zeros_bss() {
    extern "C" {
        static mut __bss_beg: u64;
        static mut __bss_end: u64;
    }

    let mut iter: *mut u64 = &mut __bss_beg;
    let end: *mut u64 = &mut __bss_end;

    while iter < end {
        write_volatile(iter, zeroed());
        iter = iter.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    zeros_bss();
    crate::main();
    kernel_api::syscall::exit();
}


struct InnerAlloc(UnsafeCell<(usize, usize)>);

unsafe impl Send for InnerAlloc {}

unsafe impl Sync for InnerAlloc {}

pub struct GlobalAllocator(InnerAlloc);

impl GlobalAllocator {
    const fn new() -> Self {
        unsafe {
            return GlobalAllocator(InnerAlloc(UnsafeCell::new((PAGE_ONE_END - 0x1000, PAGE_ONE_END)))) 
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