use core::fmt::Debug;

use alloc::boxed::Box;
use alloc::vec::Vec;
use fat32::vfat::VFatHandle;
use shim::io;
use shim::path::Path;

use aarch64;
use smoltcp::socket::SocketHandle;

use crate::{param::*, FILESYSTEM};
use crate::process::*;
use crate::traps::TrapFrame;
use crate::vm::*;
use aarch64::PState;

// use kernel_api::{OsError, OsResult};

/// Type alias for the type of a process ID.
pub type Id = u64;

/// A structure that represents the complete state of a process.
#[derive(Debug)]
pub struct Process {
    /// The saved trap frame of a process.
    pub context: Box<TrapFrame>,
    /// The memory allocation used for the process's stack.
    pub stack: Stack,
    /// The page table describing the Virtual Memory of the process
    pub vmap: Box<UserPageTable>,
    /// The scheduling state of the process.
    pub state: State,
    // Lab 5 2.C
    //// Socket handles held by the current process
    // pub sockets: Vec<SocketHandle>,
    pub files: Vec<Option<ProcessFile>>, // Open file table
    pub children: Vec<Arc<Mutex<ChildStatus>>>, // Child processes
    pub parent: Option<Arc<Mutex<ChildStatus>>>, // Parent process
}
use kernel_api::{OsResult, OsError};
use heap::align_down;
impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `None`. Otherwise returns `Some` of the new `Process`.
    pub fn new(parent: Option<Arc<Mutex<ChildStatus>>>) -> OsResult<Process> {
        let context = Box::new(TrapFrame::default());
        let stack = Stack::new().ok_or(OsError::NoMemory)?;
        let state = State::Ready;

        let upt = UserPageTable::new();
        
        let vmap = Box::new(upt);

        let mut files = Vec::new();

        // Open file descriptors 0, 1, 2 (stdin, stdout, stderr)
        files.push(Some(ProcessFile {
            handle: Arc::new(Mutex::new(Box::new(ConsoleFile))),
            offset: 0
        }));
        files.push(Some(ProcessFile {
            handle: Arc::new(Mutex::new(Box::new(ConsoleFile))),
            offset: 0
        }));
        files.push(Some(ProcessFile {
            handle: Arc::new(Mutex::new(Box::new(ConsoleFile))),
            offset: 0
        }));

        let p = Process {
            context,
            stack,
            vmap,
            state,
            files,
            children: Vec::new(),
            parent
        };

        Ok(p)
    }


    pub fn execve<P: AsRef<Path>>(process: &mut Process, pn: P, args: Vec<&str>) -> Result<(), OsError> {
        use fat32::traits::FileSystem;
        use shim::io::Read;
    
        trace!("[execve] Loading program '{}'", pn.as_ref().to_str().unwrap());
    
        // Load the program file
        let mut file = FILESYSTEM.open_file(pn).map_err(|_| {
            trace!("[execve] Error: Could not open file");
            OsError::InvalidFile
        })?;
    
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|_| {
            trace!("[execve] Error: Failed to read file");
            OsError::InvalidFile
        })?;
    
        // Allocate pages for the process image
        let data_pages = data.chunks(PAGE_SIZE);
        let page_nums = data_pages.len();
    
        for (idx, data_page) in data_pages.enumerate() {
            let va = VirtualAddr::from(Process::get_image_base().as_usize() + PAGE_SIZE * idx);
            let page = process.vmap.alloc(va, PagePerm::RWX);
            page[..data_page.len()].copy_from_slice(data_page);
        }
    
        // Allocate user heap pages
        let user_heap_pages = 16; // 1MB user heap
        for idx in page_nums..(page_nums + user_heap_pages) {
            let va = VirtualAddr::from(Process::get_image_base().as_usize() + PAGE_SIZE * idx);
            let page = process.vmap.alloc(va, PagePerm::RWX);
            page.iter_mut().for_each(|x| *x = 0);
        }

        // let mut new_tf = Box::new(TrapFrame::default());
        // new_tf.ttbr0_el1 = process.context.ttbr0_el1;
        // new_tf.ttbr1_el1 = process.vmap.get_baddr().as_u64();
        // process.context = new_tf;
        use aarch64::PState;
        let mut pstate = PState::new(0);
        pstate.set_value(0b1_u64, PState::F);
        // pstate.set_value(0b1_u64, PState::I);
        pstate.set_value(0b1_u64, PState::A);
        pstate.set_value(0b1_u64, PState::D);
        pstate.set_value(0b000_u64, PState::M); // EL0
        process.context.pstate = pstate.get();
    
        // Set stack pointer
        process.context.sp = Process::get_stack_top().as_u64();
    
        // Set process entry point
        process.context.pc = Process::get_image_base().as_u64();
    
        // --- Set up user stack ---
        let mut sp = process.context.sp as *mut u8;

        // We'll build the argument block from the bottom up.
        // We'll push the actual argument strings first (in order), then the argv array, then argc.

        // --- Step 1: Push argument strings (in normal order) ---
        // We want the strings to appear in memory in the same order as in args.
        let mut arg_ptrs = Vec::new();
        for arg in args.iter() {
            trace!("[execve] Pushing argument: {} of size {}", arg, arg.len());
            let len = arg.len() + 1; // +1 for the null terminator
            sp = unsafe { sp.sub(len) }; // allocate space for the string
            unsafe {
                // Copy the string bytes
                core::ptr::copy_nonoverlapping(arg.as_ptr(), sp, arg.len());
                // Write null terminator
                *sp.add(arg.len()) = 0;
            }
            // Record this string's address (it will be used for argv)
            arg_ptrs.push(sp as u64);
        }
        // Now, arg_ptrs[0] is the pointer to the first argument string, etc.

        // --- Step 2: Push the argv array (an array of u64 pointers) ---
        // We want to store [ arg_ptrs[0], arg_ptrs[1], ..., NULL ] on the stack.
        // First, push a null pointer as the terminator.
        let mut arg_ptrs = Vec::new();
        for arg in args.iter() {
            let len = arg.len() + 1; // +1 for null terminator.
            sp = unsafe { sp.sub(len) }; // Allocate space for the string.
            unsafe {
                core::ptr::copy_nonoverlapping(arg.as_ptr(), sp, arg.len());
                *sp.add(arg.len()) = 0; // Write null terminator.
            }
            // Save this string’s address.
            arg_ptrs.push(sp as u64);
        }
        // Now, arg_ptrs[0] is the pointer to the first argument string, etc.
    
        // --- Step 2: Push the argv array (an array of u64 pointers) ---
        // We want to create an array: [ arg_ptrs[0], arg_ptrs[1], ..., NULL ]
        // First, push a null pointer as the terminator.
        sp = unsafe { sp.sub(core::mem::size_of::<u64>()) };
        unsafe { *(sp as *mut u64) = 0 };
    
        // Then push each pointer from arg_ptrs in reverse order,
        // so that the lowest memory (highest address) gets the first argument.
        for ptr_val in arg_ptrs.iter().rev() {
            sp = unsafe { sp.sub(core::mem::size_of::<u64>()) };
            unsafe { *(sp as *mut u64) = *ptr_val };
        }
        // At this point, the argv array starts at the current sp.
        // Save its address; however, note that we haven't yet pushed argc.
        // We'll compute the final argv pointer after pushing argc.
        let temp_argv_ptr = sp as u64;
    
        // --- Step 3: Push argc onto the stack ---
        let argc = args.len() as u64;
        sp = unsafe { sp.sub(core::mem::size_of::<u64>()) };
        unsafe { *(sp as *mut u64) = argc };
    
        // Now the final stack pointer (sp) points to argc.
        // According to the AArch64 C ABI, main expects:
        //   x0 = argc (the value at SP)
        //   x1 = pointer to argv, which is at (SP + 8)
        let final_sp = sp;  // final_sp is where argc is stored.
        let final_argv_ptr = unsafe { final_sp.add(core::mem::size_of::<u64>()) } as u64;
    
        // --- Step 4: Align the final stack pointer to a 16-byte boundary ---
        let sp_aligned = (final_sp as usize & !0xF) as *mut u8;
        process.context.sp = sp_aligned as u64;
    
        // --- Step 5: Update registers: x0 = argc, x1 = argv_ptr ---
        process.context.regs[0] = argc;
        process.context.regs[1] = final_argv_ptr;
    
        trace!("[execve] Stack set up: argc = {}, argv_ptr = {:#x}", argc, final_argv_ptr);

        Ok(())
    }
    

    /// Loads a program stored in the given path by calling `do_load()` method.
    /// Sets trapframe `context` corresponding to its page table.
    /// `sp` - the address of stack top
    /// `elr` - the address of image base.
    /// `ttbr0` - the base address of kernel page table
    /// `ttbr1` - the base address of user page table
    /// `spsr` - `F`, `A`, `D` bit should be set.
    ///
    /// Returns Os Error if do_load fails.
    pub fn load<P: AsRef<Path>>(pn: P, parent: Option<Arc<Mutex<ChildStatus>>>) -> OsResult<Process> {
        use crate::VMM;

        let mut p = Process::do_load(pn, parent)?;
        

        p.context.sp = Process::get_stack_top().as_u64();
        p.context.pc = Process::get_image_base().as_u64();
        p.context.ttbr0_el1 = VMM.get_baddr().as_u64();
        p.context.ttbr1_el1 = p.vmap.get_baddr().as_u64();
        use aarch64::PState;
        let mut pstate = PState::new(0);
        pstate.set_value(0b1_u64, PState::F);
        // pstate.set_value(0b1_u64, PState::I);
        pstate.set_value(0b1_u64, PState::A);
        pstate.set_value(0b1_u64, PState::D);
        pstate.set_value(0b000_u64, PState::M); // EL0
        p.context.pstate = pstate.get();

        Ok(p)
    }

    /// Creates a process and open a file with given path.
    /// Allocates one page for stack with read/write permission, and N pages with read/write/execute
    /// permission to load file's contents.
    fn do_load<P: AsRef<Path>>(pn: P, parent: Option<Arc<Mutex<ChildStatus>>>) -> OsResult<Process> {
        use fat32::traits::FileSystem;
        use shim::io::Read;
        let mut file = FILESYSTEM.open_file(pn)?;
        let mut p = Process::new(parent).expect("failed to create processs");
        p.vmap.alloc(Process::get_stack_base(), PagePerm::RWX); // allocate one page for stack
        
        use alloc::vec;
        let mut data = vec![];
        file.read_to_end(&mut data)?;

        let data_pages = data.chunks(PAGE_SIZE);
        let page_nums = data_pages.len();

        for (idx, data_page) in data_pages.enumerate() {
            let va = VirtualAddr::from(Process::get_image_base().as_usize()+PAGE_SIZE*idx);
            let page = p.vmap.alloc(va, PagePerm::RWX);
            page[..data_page.len()].copy_from_slice(data_page);
        }


        // alloc some pages for user heap
        // TODO: add page fault handler to automatically handle this
        let user_heap_pages = 16; // user can allocate 1 MB heap
        for idx in (page_nums)..(page_nums+user_heap_pages) {
            let va = VirtualAddr::from(Process::get_image_base().as_usize()+PAGE_SIZE*idx);
            p.vmap.alloc(va, PagePerm::RWX);
        }

        Ok(p)
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        let max = !0x0_u64;
        VirtualAddr::from(max)
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE)
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        VirtualAddr::from(align_down(Process::get_max_va().as_usize(), PAGE_SIZE))
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        VirtualAddr::from(align_down(Process::get_max_va().as_usize(), 0x80))
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occured. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        let state = core::mem::replace(&mut self.state, State::Dead); // Temporarily remove state

        if let State::Waiting(mut f) = state {
            if let Some(func) = f.as_mut() && func(self) {
                self.state = State::Ready;
                return true;
            } else {
                self.state = State::Waiting(f); // Restore the waiting state
                return false;
            }
        }
    
        let is_ready = matches!(state, State::Ready);
        self.state = state; // Restore state for non-waiting cases
        is_ready
    }
    
}

impl Clone for Process {
    fn clone(&self) -> Self {
        Process {
            context: self.context.clone(),
            stack: self.stack.clone(),
            vmap: self.vmap.clone(),
            state: State::Ready,
            files : self.files.clone(),
            children: Vec::new(),
            parent: None
        }
    }
}
