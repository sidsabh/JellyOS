use pi::atags::Atags;
use crate::align_up;

extern "C" {
    static __text_end: u8;
}

/// Returns the (start address, end address) of the available memory on this
/// system if it can be determined. If it cannot, `None` is returned.
///
/// This function is expected to return `Some` under all normal cirumstances.
pub fn memory_map() -> Option<(usize, usize)> {
    
    let page_size: usize = 1 << 12;
    let mut binary_end = unsafe { (&__text_end as *const u8) as usize };
    binary_end = align_up(binary_end, page_size);
    let mut atags = Atags::get();
    match atags.find(|tag| tag.mem().is_some()) {
        Some(atag) => {
            let mem = atag.mem().unwrap();
            Some((binary_end, (mem.size as usize) - binary_end))
        }
        None => Some((1000000, 1006022656)) // atags not appearing for ELF kernel QEMU fix,
        // None => None // correct code
    }

}

