use core::fmt;
use shim::const_assert_size;

use crate::traits::BlockDevice;
use crate::vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    jmp_instruction: [u8; 3],
    oem_identifier: u64,
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    num_reserved_sectors: u16,
    num_fats: u8,
    max_dir_entries: u16,
    small_total_logical_sectors: u16,
    fat_id: u8,
    old_spf: u16, // sectors per fat previous fat32
    sectors_per_track: u16,
    num_heads: u16,
    num_hidden_scectors : u32, // LBA of the beginning of the partition
    num_logical_sectors : u32,
    sectors_per_fat : u32,
    flags : u16,
    version : u16,
    root_dir_cluster : u32,
    fsinfo_sector : u16,
    backup_boot_sector : u16,
    _reserved : [u8; 12],
    drive_num : u8,
    _reserved_win : u8,
    signature : u8,
    volume_id : u32,
    volume_label : [u8; 11],
    sys_identifier: u64,
    boot_code : [u8; 420],
    boot_signature : u16
    // pub start: u64,
    // pub num_sectors: u64,
    // pub sector_size: u64,
}

// impl Default for BiosParameterBlock {
//     fn default() -> BiosParameterBlock {
//         BiosParameterBlock {}
//     }
// }

const_assert_size!(BiosParameterBlock, 512);

impl BiosParameterBlock {
    /// Reads the FAT32 extended BIOS parameter block from sector `sector` of
    /// device `device`.
    ///
    /// # Errors
    ///
    /// If the EBPB signature is invalid, returns an error of `BadSignature`.
    pub fn from<T: BlockDevice>(mut device: T, sector: u64) -> Result<BiosParameterBlock, Error> {
        unimplemented!("BiosParameterBlock::from()")
    }
}

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!("BiosParameterBlock::fmt()")
    }
}
