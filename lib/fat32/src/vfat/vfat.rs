use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::Path;

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

/// A generic trait that handles a critical section as a closure
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    rootdir_cluster: Cluster,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
    where
        T: BlockDevice + 'static,
    {
        let mbr = MasterBootRecord::from(&mut device)?;
        let p_entry = mbr.get_fat32_partition()?;
        let bpb = BiosParameterBlock::from(&mut device, p_entry.relative_sector as u64)?;
        let partition = Partition {
            start: p_entry.relative_sector as u64,
            num_sectors: p_entry.total_sectors as u64,
            sector_size: bpb.bytes_per_sector as u64,
        };

        let cp = CachedPartition::new(device, partition);

        let dss: u64 =
            (bpb.num_reserved_sectors as u64) + (bpb.num_fats as u32 * bpb.sectors_per_fat) as u64;
        let vfat: VFat<HANDLE> = VFat {
            phantom: PhantomData,
            device: cp,
            bytes_per_sector: bpb.bytes_per_sector,
            sectors_per_cluster: bpb.sectors_per_cluster,
            sectors_per_fat: bpb.sectors_per_fat,
            fat_start_sector: bpb.num_reserved_sectors as u64,
            data_start_sector: dss,
            rootdir_cluster: Cluster::from(bpb.root_dir_cluster),
        };
        Ok(HANDLE::new(vfat))
    }

    // Recommended
    fn cluster_start_sector(&mut self, cluster: Cluster) -> io::Result<u64> {
        let cluster_num: u32 = cluster.into();
        let sfc = self.data_start_sector + ((cluster_num as u64) * self.sectors_per_cluster as u64);
        // add error checking (return a sector out of range)
        Ok(sfc)
    }

    //
    //  * A method to read from an offset of a cluster into a buffer.
    //
    fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        let mut curr_sector =
            self.cluster_start_sector(cluster)? + offset as u64 / self.bytes_per_sector as u64;
        let mut curr_offset = offset % (self.bytes_per_sector as usize);

        let mut bytes_read = 0;
        for _ in 0..self.sectors_per_cluster {
            bytes_read += self
                .device
                .read_sector(curr_sector, &mut buf[curr_offset..])?;

            curr_offset += bytes_read;
            curr_sector += 1;
        }
        Ok(bytes_read)
    }
    //
    //  * A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    //
    fn read_chain(&mut self, start: Cluster, buf: &mut Vec<u8>) -> io::Result<usize> {

        let mut curr_cluster = start;
        let mut bytes_read = 0;
        
        while let Status::Data(cluster) = self.fat_entry(curr_cluster)?.status() {
            bytes_read += self.read_cluster(cluster, 0, buf)?;

            let next_entry = self.fat_entry(cluster)?.0;
            curr_cluster = Cluster::from(next_entry);
        }
        Ok(bytes_read)
    }
    //
    //  * A method to return a reference to a `FatEntry` for a cluster where the
    //    reference points directly into a cached sector.
    //
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let cluster_num: u32 = cluster.into();
        let entries_per_sector = self.bytes_per_sector / size_of::<u32>() as u16;
        let fat_sector = self.fat_start_sector + (cluster_num as u64 / entries_per_sector as u64);
        let mut buf = vec![0 as u8; self.bytes_per_sector as usize];
        self.device.read_sector(fat_sector, &mut buf)?;
        let mod_buf = unsafe { buf.cast::<u32>() };
        let fv: &u32 = &mod_buf[(cluster_num as usize) % entries_per_sector as usize];
        let fat_entry: &FatEntry = unsafe { &*(fv as *const u32 as *const FatEntry) };
        Ok(fat_entry)
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::traits::Dummy;
    type Dir = crate::traits::Dummy;
    type Entry = crate::traits::Dummy;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        unimplemented!("FileSystem::open()")
    }
}
