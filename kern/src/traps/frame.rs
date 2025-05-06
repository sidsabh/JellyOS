use core::fmt;
use shim::const_assert_size;

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct TrapFrame {
    pub pc : u64, // User PC when exception occurs
    pub pstate : u64,
    pub sp : u64, // User SP when exception occurs
    pub tpidr : u64,
    pub ttbr0_el1 : u64, // Kernel page table base register
    pub ttbr1_el1 : u64, // User page table base register
    big_regs : [u128; 32],
    pub regs : [u64; 31],
    reserved : u64,
}

const_assert_size!(TrapFrame, 816);

impl fmt::Debug for TrapFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TrapFrame {{ pc: {:016x}, pstate: {:016x}, sp: {:016x}, tpidr: {:016x}, ttbr0_el1: {:016x}, ttbr1_el1: {:016x}", self.pc, self.pstate, self.sp, self.tpidr, self.ttbr0_el1, self.ttbr1_el1)?;
        for i in 0..31 {
            write!(f, ", x{}: {:016x}", i, self.regs[i])?;
        }
        write!(f, " }}")
        
        
    }
}