use core::iter::Chain;
use core::ops::{Deref, DerefMut, SubAssign};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator::{self, memory_map, Allocator};
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
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 2],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut pt = PageTable {
            l2: L2PageTable::new(),
            l3: [L3PageTable::new(), L3PageTable::new()],
        };

        for (i, entry) in pt.l3.iter_mut().enumerate() {
            pt.l2.entries[i].set_value(EntryValid::Valid, RawL2Entry::VALID);
            // type=1 gives the address of the next level of translation table, and some attributes for that translation
            pt.l2.entries[i].set_value(EntryType::Table, RawL2Entry::TYPE);
            pt.l2.entries[i].set_value(EntryAttr::Mem, RawL2Entry::ATTR);
            pt.l2.entries[i].set_value(perm, RawL2Entry::AP);
            pt.l2.entries[i].set_value(EntrySh::ISh, RawL2Entry::SH);
            pt.l2.entries[i].set_value(entry.as_ptr().as_u64(), RawL2Entry::ADDR);
        }

        Box::new(pt)
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// Since we are only supporting 1GB virtual memory in this system, L2index
    /// should be smaller than 2.
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
        PhysicalAddr::from(self as *const PageTable as *const u64 as u64)
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
    type IntoIter = Chain<IterMut<'a, L3Entry>, IterMut<'a, L3Entry>>;

    fn into_iter(self) -> Self::IntoIter {
        let (first, second) = self.l3.split_at_mut(1);
        let iter1 = first[0].entries.iter_mut();
        let iter2 = second[0].entries.iter_mut();
        iter1.chain(iter2)
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

        let mut idx = start / PAGE_SIZE;
        // identity map physical memory
        for addr in (start..end).step_by(PAGE_SIZE) {
            let mut entry = RawL3Entry::new(0);
            entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            entry.set_value(PageType::Page, RawL3Entry::TYPE);
            entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntrySh::ISh, RawL3Entry::SH);
            entry.set_value(addr as u64, RawL3Entry::ADDR);

            *entries[idx] = L3Entry(entry);
            idx += 1;
        }

        // identity map mmio
        idx = IO_BASE / PAGE_SIZE;
        for addr in (IO_BASE..IO_BASE_END).step_by(PAGE_SIZE) {
            let mut entry = RawL3Entry::new(0);
            entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            entry.set_value(PageType::Page, RawL3Entry::TYPE);
            entry.set_value(EntryAttr::Dev, RawL3Entry::ATTR);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntrySh::OSh, RawL3Entry::SH);
            entry.set_value(addr as u64, RawL3Entry::ADDR);

            *entries[idx] = L3Entry(entry);
            idx += 1;
        }

        kpt
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

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
            panic!("virtual address is lower than `USER_IMG_BASE`");
        } 
        
        va.sub_assign(USER_IMG_BASE);
        if self.is_valid(va) {
            panic!("virtual address has already been allocated");
        }

        let page = unsafe {
            ALLOCATOR.alloc(Page::layout())
        };
        if page == core::ptr::null_mut() {
            panic!("allocator failed to allocate a page")
        }





        let page = unsafe {
            ALLOCATOR.alloc(Page::layout())
        };

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

// FIXME: Implement `Drop` for `UserPageTable`.
// FIXME: Implement `fmt::Debug` as you need.
