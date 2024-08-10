pub mod sd;

use alloc::rc::Rc;
use core::fmt::{self, Debug};
use fat32::traits::Entry;
use shim::io;
use shim::ioerr;
use shim::path::Path;

pub use fat32::traits;
use fat32::vfat::{Dir, Entry as EntryStruct, File, VFat, VFatHandle};

use self::sd::Sd;
use crate::console::kprint;
use crate::mutex::Mutex;

#[derive(Clone)]
pub struct PiVFatHandle(Rc<Mutex<VFat<Self>>>);

// These impls are *unsound*. We should use `Arc` instead of `Rc` to implement
// `Sync` and `Send` trait for `PiVFatHandle`. However, `Arc` uses atomic memory
// access, which requires MMU to be initialized on ARM architecture. Since we
// have enabled only one core of the board, these unsound impls will not cause
// any immediate harm for now. We will fix this in the future.
unsafe impl Send for PiVFatHandle {}
unsafe impl Sync for PiVFatHandle {}

impl Debug for PiVFatHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "PiVFatHandle")
    }
}

impl VFatHandle for PiVFatHandle {
    fn new(val: VFat<PiVFatHandle>) -> Self {
        PiVFatHandle(Rc::new(Mutex::new(val)))
    }

    fn lock<R>(&self, f: impl FnOnce(&mut VFat<PiVFatHandle>) -> R) -> R {
        f(&mut self.0.lock())
    }
}
pub struct FileSystem(Mutex<Option<PiVFatHandle>>);

impl FileSystem {
    /// Returns an uninitialized `FileSystem`.
    ///
    /// The file system must be initialized by calling `initialize()` before the
    /// first memory allocation. Failure to do will result in panics.
    pub const fn uninitialized() -> Self {
        FileSystem(Mutex::new(None))
    }

    /// Initializes the file system.
    /// The caller should assure that the method is invoked only once during the
    /// kernel initialization.
    ///
    /// # Panics
    ///
    /// Panics if the underlying disk or file sytem failed to initialize.
    pub unsafe fn initialize(&self) {
        let mut t = self.0.lock();
        let sd_card = sd::Sd::new().expect("sd card failed to load");
        let fs = VFat::from(sd_card).expect("failed to make fs");
        let _handle = t.insert(fs);
    }
}

// FIXME: Implement `fat32::traits::FileSystem` for `&FileSystem`
impl fat32::traits::FileSystem for &FileSystem {
    type File = File<PiVFatHandle>;

    type Dir = Dir<PiVFatHandle>;

    type Entry = EntryStruct<PiVFatHandle>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        let new_handler: PiVFatHandle;
        {
            let guard = self.0.lock();
            new_handler = guard.clone().expect("failed to get handler");
        }
        let root_dir: Dir<PiVFatHandle>;
        {
            root_dir = new_handler.0.lock().get_root_dir(&new_handler)?;
        }
        let mut curr = EntryStruct::DirEntry(root_dir);
        for component in path.as_ref().components().skip(1) {
            match component {
                // path::Component::Prefix(_) => todo!(),
                // path::Component::RootDir => todo!(),
                // path::Component::CurDir => todo!(),
                // path::Component::ParentDir => todo!(),
                shim::path::Component::Normal(name) => {
                    if let Some(dir) = curr.as_dir() {
                        curr = dir.find(name)?;
                    } else {
                        return Err(io::Error::new(io::ErrorKind::NotFound, "Path not found"));
                    }
                }
                _ => return Err(io::Error::new(io::ErrorKind::NotFound, "Path not found")),
            }
        }
        Ok(curr)
    }
}
