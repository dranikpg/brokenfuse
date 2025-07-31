use fuser::{FileAttr, FileType};
use std::rc::Rc;
use crate::storage::Storage;
use crate::effect::{EffectGroup};
pub type Ino = usize;
pub type ErrNo = libc::c_int;

#[derive(Default)]
pub struct Dir {
    children: Vec<(Ino, String)>,
}

impl Dir {
    // Find entry by name
    pub fn lookup(&self, name: &(impl PartialEq<str> + ?Sized)) -> Option<Ino> {
        self.children
            .iter()
            .filter(|(_, fname)| name == fname.as_str())
            .map(|(fino, _)| *fino)
            .next()
    }

    // List all entries in undefined order
    pub fn list(&self) -> impl Iterator<Item = (Ino, &str)> {
        self.children
            .iter()
            .map(|(fino, fname)| (*fino, fname.as_str()))
    }

    // Add entry
    pub fn add(&mut self, ino: Ino, name: String) {
        self.children.push((ino, name))
    }

    // Remove entry and return if removed
    pub fn remove(&mut self, ino: Ino) {
        self.children.retain(|(i, _n)| *i != ino);
    }
}

pub struct File {
    storage: Box<dyn Storage>,
}

impl File {
    pub fn create(storage: Box<dyn Storage>) -> File {
        File { storage }
    }

    pub fn storage(&self) -> &dyn Storage {
        self.storage.as_ref()
    }

    pub fn storage_mut(&mut self) -> &mut dyn Storage {
        self.storage.as_mut()
    }
}

pub enum NodeItem {
    File(File),
    Dir(Dir),
}

impl NodeItem {
    pub fn ftype(&self) -> FileType {
        match self {
            NodeItem::Dir(d) => FileType::Directory,
            NodeItem::File(f) => FileType::RegularFile
        }
    }
}

pub struct Node {
    pub parent: usize,
    pub attr: FileAttr,
    pub item: NodeItem,
    pub effects: Option<Rc<EffectGroup>>
}
