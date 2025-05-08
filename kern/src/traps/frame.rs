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
    pub ksp : u64, // Kernel SP when exception occurs
}

impl TrapFrame {
    pub fn is_idle(&self) -> bool {
        self.tpidr == u64::MAX
    }
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
use alloc::boxed::Box;

// pub fn get_current_trap_frame() -> &'static TrapFrame {
//     let mut tf = Box::new(TrapFrame::default());
//     let tf_ptr = Box::into_raw(tf);

//     unsafe {
//         core::arch::asm!(
//             // --- 1) save original SP on the stack ---
//             "mov  x1, sp",
//             "str  x1, [sp, #-16]!",

//             // --- 2) switch SP to our new frame and call vec_context_save ---
//             "mov  x0, {frame}",      // x0 = &*tf_ptr
//             "mov  sp, x0",
//             "bl   vec_context_save",

//             // --- 3) restore original SP from stack into x1 ---
//             "ldr  x1, [sp], #16",

//             // --- 4) write x1 into TrapFrame.sp (offset 16) ---
//             "mov  x2, {frame}",      // x2 = &*tf_ptr
//             "str  x1, [x2, #16]",

//             // --- 5) restore SP so the CPU stack is back to what it was ---
//             "mov  sp, x1",

//             frame = in(reg) tf_ptr,
//             out("x0") _, out("x1") _, out("x2") _,
//         );
//     }

//     unsafe { &*tf_ptr }
// }
