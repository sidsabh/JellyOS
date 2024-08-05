use alloc::string::String;
use alloc::vec::Vec;

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;

use crate::traits;
use crate::traits::Dir as DirTrait;
use crate::traits::Entry as EntryTrait;
use crate::util::SliceExt;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{CachedPartition, Cluster, Entry, File, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,           // file system handle
    pub first_cluster: Cluster, // first cluster
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatRegularDirEntry {
    file_name: u64,
    file_extension: [u8; 3],
    file_attributes: Attributes,
    reserved_win: u8,
    creation_secs_tenths: u8,
    creation_time: Time,
    creation_date: Date,
    accessed_date: Date,
    high_cluster_num: u16,
    modification_time: Time,
    modification_date: Date,
    low_cluster_num: u16,
    file_size: u32,
}

const_assert_size!(VFatRegularDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatLfnDirEntry {
    sequence_num: u8,
    first_name_chars: [u8; 10],
    file_attributes: Attributes,
    file_type: u8,
    checksum: u8,
    second_name_chars: [u8; 12],
    zeroes: u16,
    third_name_chars: [u8; 4],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatUnknownDirEntry {
    // ID of 0x00. Indicates the end of the directory.
    /// ID of 0xE5: Marks an unused/deleted entry.
    /// All other IDs make up part of the file’s name or LFN sequence number.
    file_id: u8,
    _reserved : [u8; 10],
    /// The byte at offset 11 determines whether the entry is a regular entry or an LFN entry.
    /// Value of 0x0F: entry is an LFN entry.
    /// All other values: entry is a regular entry.
    reg_or_lfn: u8,
    _reserved2 : [u8; 20]
}

const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        for entry in self.entries()? {
            if OsStr::new(entry.name()).eq_ignore_ascii_case(name.as_ref()) { // bro what is this
                return Ok(entry);
            }
        }
        return Err(io::Error::new(io::ErrorKind::NotFound, "no entry with that name"));
    }
}


pub struct DirIterator<HANDLE: VFatHandle> {
    directory_data: Vec<u8>,
    index: usize,
    vfat: HANDLE,
}

use core::mem::size_of;
impl<HANDLE: VFatHandle> Iterator for DirIterator<HANDLE> {
    type Item = Entry<HANDLE>;
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: might want to change to using some refs for the diriterator instead of reading entire thing?

        let idx = size_of::<VFatDirEntry>()*self.index;
        let data = &self.directory_data[idx..idx+1];
        unsafe {
            let entry: &[VFatDirEntry] = data.cast::<VFatDirEntry>();
            let real = &entry[0];
        }
        // let e = self.directory_data
        self.index += 1;

        None

    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    type Entry = Entry<HANDLE>;
    type Iter = DirIterator<HANDLE>;

    fn entries(&self) -> io::Result<Self::Iter> {
        // . You will likely need to use at-most one line of unsafe when implementing entries(); you may find the VecExt and SliceExt trait implementations we have provided particularly useful here.
        // data.cast() ???
        let mut data: Vec<u8> = vec![];
        self.vfat.lock(|s|{
            s.read_chain(self.first_cluster, &mut data)
        })?;
        Ok(DirIterator {
            directory_data: data,
            index: 0,
            vfat: self.vfat.clone()
        })
    }
}
