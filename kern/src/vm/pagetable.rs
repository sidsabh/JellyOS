use core::iter::Chain;
use core::ops::{Deref, DerefMut, SubAssign};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator::{self, memory_map };
use crate::console::{kprint, kprintln};
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;

use aarch64::vmsa::*;
use shim::const_assert_size;

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
#[derive(Copy, Clone)]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// Returns a new `L2PageTable`
    fn new() -> L2PageTable {
        L2PageTable {
            entries: [RawL2Entry::new(0); 8192],
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L2PageTable as *const u64 as u64)
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// Returns a new `L3Entry`.
    fn new() -> L3Entry {
        L3Entry(RawL3Entry::new(0))
    }

    /// Returns `true` if the L3Entry is valid and `false` otherwise.
    fn is_valid(&self) -> bool {
        self.0.get_value(RawL3Entry::VALID) == 0x1
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(PhysicalAddr::from(self.0.get_masked(RawL3Entry::ADDR)))
        } else {
            None
        }
    }
}

#[repr(C)]
#[repr(align(65536))]
#[derive(Copy, Clone)]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// Returns a new `L3PageTable`.
    fn new() -> L3PageTable {
        L3PageTable {
            entries: [L3Entry::new(); 8192],
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L3PageTable as *const u64 as u64)
    }
}

#[repr(C)]
#[repr(align(65536))]
#[derive(Copy, Clone)]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 3],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut pt = Box::new(PageTable {
            l2: L2PageTable::new(),
            // hard coded to 3 PTs, each of 8K entries of 64KB pages
            // 3 * 2^13 * 2^16 = 3 * 2^29
            l3: [L3PageTable::new(), L3PageTable::new(), L3PageTable::new()],
        });

        for (i, entry) in pt.l3.iter().enumerate() {
            pt.l2.entries[i].set_value(EntryValid::Valid, RawL2Entry::VALID);
            pt.l2.entries[i].set_value(EntryType::Table, RawL2Entry::TYPE);
            pt.l2.entries[i].set_value(EntryAttr::Mem, RawL2Entry::ATTR);
            pt.l2.entries[i].set_value(perm, RawL2Entry::AP);
            pt.l2.entries[i].set_value(EntrySh::ISh, RawL2Entry::SH);
            pt.l2.entries[i].set_value(0b1_u64, RawL2Entry::AF);
            pt.l2.entries[i].set_value(entry.as_ptr().as_u64() >> PAGE_ALIGN, RawL2Entry::ADDR);
        }
        
        pt
    }


    pub fn get_page(&self, va: VirtualAddr) -> Option<&mut [u8]> {
        let (l2_idx, l3_idx) = PageTable::locate(va);
        let entry = &self.l3[l2_idx].entries[l3_idx];
        if entry.is_valid() {
            let addr = entry.0.get_masked(RawL3Entry::ADDR) as *mut u8;
            Some(unsafe { core::slice::from_raw_parts_mut(addr, Page::SIZE) })
        } else {
            None
        }
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// L2index should be smaller than the number of L3PageTable.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let l2_idx = (va.as_usize() >> 29) & 0x1;
        let l3_idx = (va.as_usize() >> 16) & 0x1FFF;
        assert!(l2_idx < 2);
        (l2_idx, l3_idx)
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_idx, l3_idx) = PageTable::locate(va);
        self.l3[l2_idx].entries[l3_idx].is_valid()
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is invalid.
    /// Otherwise, `false` is returned.
    pub fn is_invalid(&self, va: VirtualAddr) -> bool {
        !self.is_valid(va)
    }

    /// Set the given RawL3Entry `entry` to the L3Entry indicated by the given virtual
    /// address.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2_idx, l3_idx) = PageTable::locate(va);
        self.l3[l2_idx].entries[l3_idx] = L3Entry(entry);
        self
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const _ as u64)
    }
}

impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;

    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    fn into_iter(self) -> Self::IntoIter {
        self.l3[0].entries.iter().chain(self.l3[1].entries.iter())
    }
}

use core::slice::IterMut;

impl<'a> IntoIterator for &'a mut PageTable {
    type Item = &'a mut L3Entry;
    type IntoIter = core::iter::FlatMap<
        core::slice::IterMut<'a, L3PageTable>,
        IterMut<'a, L3Entry>,
        fn(&'a mut L3PageTable) -> IterMut<'a, L3Entry>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.l3.iter_mut().flat_map(|table| table.entries.iter_mut())
    }
}

pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// Returns a new `KernPageTable`. `KernPageTable` should have a `Pagetable`
    /// created with `KERN_RW` permission.
    ///
    /// Set L3entry of ARM physical address starting at 0x00000000 for RAM and
    /// physical address range from `IO_BASE` to `IO_BASE_END` for peripherals.
    /// Each L3 entry should have correct value for lower attributes[10:0] as well
    /// as address[47:16]. Refer to the definition of `RawL3Entry` in `vmsa.rs` for
    /// more details.
    pub fn new() -> KernPageTable {
        let mut kpt = KernPageTable(PageTable::new(EntryPerm::KERN_RW));

        let start: usize = 0x0;
        let (_, end): (usize, usize) = memory_map().expect("failed to load memory map");

        use alloc::vec::Vec;

        let mut entries: Vec<&mut L3Entry> = kpt
            .l3
            .iter_mut()
            .flat_map(|l3_table| l3_table.entries.iter_mut())
            .collect();

        // identity map physical memory
        for (idx, addr) in (start..end).step_by(PAGE_SIZE).enumerate() {
            let mut entry = RawL3Entry::new(0);
            entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            entry.set_value(PageType::Page, RawL3Entry::TYPE);
            entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntrySh::ISh, RawL3Entry::SH);
            entry.set_value(0b1_u64, RawL3Entry::AF);
            entry.set_masked(addr as u64, RawL3Entry::ADDR);

            *entries[idx] = L3Entry(entry);
        }

        // identity map mmio
        for (idx, addr) in (IO_BASE..IO_BASE_END).step_by(PAGE_SIZE).enumerate() {
            let mut entry = RawL3Entry::new(0);
            entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            entry.set_value(PageType::Page, RawL3Entry::TYPE);
            entry.set_value(EntryAttr::Dev, RawL3Entry::ATTR);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntrySh::OSh, RawL3Entry::SH);
            entry.set_value(0b1_u64, RawL3Entry::AF);
            entry.set_masked(addr as u64, RawL3Entry::ADDR);

            *entries[idx + IO_BASE / PAGE_SIZE] = L3Entry(entry);
        }


        kpt
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

#[derive(Debug)]
pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        UserPageTable(PageTable::new(EntryPerm::USER_RW))
    }

    /// Allocates a page and set an L3 entry translates given virtual address to the
    /// physical address of the allocated page. Returns the allocated page.
    ///
    /// # Panics
    /// Panics if the virtual address is lower than `USER_IMG_BASE`.
    /// Panics if the virtual address has already been allocated.
    /// Panics if allocator fails to allocate a page.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        if va.as_usize() < USER_IMG_BASE {
            panic!("virtual address {} is lower than `USER_IMG_BASE`", va.as_usize());
        }

        use core::ops::Sub;
        let adj_va = va.sub(VirtualAddr::from(USER_IMG_BASE)); // normalize

        if self.is_valid(adj_va) {
            return self.get_page(va).unwrap();
            // panic!("virtual address has already been allocated");
        }

        use core::slice;
        let page: *mut u8;
        let page_slice: &mut [u8];
        unsafe {
            page = ALLOCATOR.alloc(Page::layout());
            page_slice = slice::from_raw_parts_mut(page, Page::SIZE);
        };

        if page == core::ptr::null_mut() {
            panic!("allocator failed to allocate a page")
        };

        let mut entry = RawL3Entry::new(0);
        entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
        entry.set_value(PageType::Page, RawL3Entry::TYPE);
        entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        entry.set_value(EntryPerm::USER_RW, RawL3Entry::AP); // use _perm ?
        entry.set_value(EntrySh::ISh, RawL3Entry::SH);
        entry.set_value(0b1_u64, RawL3Entry::AF);
        entry.set_masked(page as u64, RawL3Entry::ADDR);

        self.set_entry(adj_va, entry);

        page_slice
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for UserPageTable {
    fn drop(&mut self) {
        for entry in self.into_iter() {
            if entry.is_valid() {
                let ptr = entry.0.get_masked(RawL3Entry::ADDR) as *mut u8;
                unsafe {
                    ALLOCATOR.dealloc(ptr, Page::layout());
                }
            }
        }
    }
}

impl fmt::Debug for L2PageTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("L2PageTable")
            .field("entries", &self.entries)
            .finish()
    }
}

impl fmt::Debug for L3PageTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("L3PageTable")
            .field("entries", &self.entries.map(|e| e.0))
            .finish()
    }
}

impl fmt::Debug for PageTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTable")
            .field("l2", &self.l2)
            .field("l3", &self.l3)
            .finish()
    }
}



impl UserPageTable {
    fn get_page_slice(&self, va: VirtualAddr) -> &[u8] {
        let (l2_idx, l3_idx) = PageTable::locate(va);
        let entry = &self.l3[l2_idx].entries[l3_idx];
        if entry.is_valid() {
            let addr = entry.0.get_masked(RawL3Entry::ADDR) as *const u8;
            unsafe { core::slice::from_raw_parts(addr, Page::SIZE) }
        } else {
            panic!("Invalid page entry");
        }
    }
}

impl Clone for UserPageTable {
    fn clone(&self) -> Self {
        let mut new_page_table = UserPageTable::new();

        for (l2_idx, l3_table) in self.l3.iter().enumerate() {
            for (l3_idx, entry) in l3_table.entries.iter().enumerate() {
                if entry.is_valid() {
                    let old_phys_addr = entry.0.get_masked(RawL3Entry::ADDR);
                    let virt_addr = VirtualAddr::from(USER_IMG_BASE+ (l2_idx << 29) | (l3_idx << 16));

                    trace!("Cloning page at {:x} to {:x}", old_phys_addr, virt_addr.as_usize());

                    // Allocate a new page
                    let new_page_slice = new_page_table.alloc(virt_addr, PagePerm::RWX);

                    // Get the old page slice
                    let old_page_slice = self.get_page_slice(virt_addr);

                    // Copy the data
                    new_page_slice.copy_from_slice(old_page_slice);
                }
            }
        }

        new_page_table
    }
}
