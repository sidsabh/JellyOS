use crate::traits;
use crate::vfat::{Dir as DirStruct, File as FileStruct, Metadata, VFatHandle};


// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    FileEntry(FileStruct<HANDLE>),
    DirEntry(DirStruct<HANDLE>),
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    type File = FileStruct<HANDLE>;

    type Dir = DirStruct<HANDLE>;

    type Metadata = Metadata;

    fn name(&self) -> &str {
        todo!()
    }

    fn metadata(&self) -> &Self::Metadata {
        todo!()
    }

    fn as_file(&self) -> Option<&Self::File> {
        todo!()
    }

    fn as_dir(&self) -> Option<&Self::Dir> {
        todo!()
    }

    fn into_file(self) -> Option<Self::File> {
        todo!()
    }

    fn into_dir(self) -> Option<Self::Dir> {
        todo!()
    }
}
