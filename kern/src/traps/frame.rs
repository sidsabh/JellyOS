use core::fmt;
use shim::const_assert_size;

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct TrapFrame {
    pub pc : u64,
    pub pstate : u64,
    pub sp : u64,
    pub tpidr : u64,
    big_regs : [u128; 32],
    regs : [u64; 31],
    reserved : u64
}

const_assert_size!(TrapFrame, 800);
