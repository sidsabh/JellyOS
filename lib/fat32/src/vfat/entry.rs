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
        match &self {
            Entry::FileEntry(fe) => fe.name.as_str(),
            Entry::DirEntry(de) => de.name.as_str(),
        }
    }

    fn metadata(&self) -> &Self::Metadata {
        match &self {
            Entry::FileEntry(fe) => &fe.metadata,
            Entry::DirEntry(de) => &de.metadata.as_ref().unwrap(),
        }
    }

    fn as_file(&self) -> Option<&Self::File> {
        if let Self::FileEntry(fe) = &self {
            Some(fe)
        } else {
            None
        }
    }

    fn as_dir(&self) -> Option<&Self::Dir> {
        if let Self::DirEntry(de) = &self {
            Some(de)
        } else {
            None
        }
    }

    fn into_file(self) -> Option<Self::File> {
        if let Self::FileEntry(fe) = self {
            Some(fe)
        } else {
            None
        }
    }

    fn into_dir(self) -> Option<Self::Dir> {
        if let Self::DirEntry(de) = self {
            Some(de)
        } else {
            None
        }
    }
}
