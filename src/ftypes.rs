use fuser::FileAttr;
use serde::Serialize;
use std::cell::Cell;

use crate::effect::Group;
use crate::storage::Storage;

pub type Ino = usize;
pub type ErrNo = libc::c_int;

// Dir manages a list of children. It does NOT manage the nodes lifetimes
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

    // Remove entry and return removed inode
    pub fn remove(&mut self, name: &(impl PartialEq<str> + ?Sized)) -> Option<Ino> {
        let ino = self.lookup(name)?;
        self.children.retain(|(_, fname)| name != fname.as_str());
        Some(ino)
    }
}

#[derive(Default, Serialize)]
pub struct FileStats {
    pub reads: Cell<usize>,
    pub read_volume: Cell<usize>,
    pub writes: Cell<usize>,
    pub write_volume: Cell<usize>,
    pub errors: Cell<usize>,
}

pub struct File {
    storage: Box<dyn Storage>,
    pub stats: FileStats,
}

impl File {
    pub fn create(storage: Box<dyn Storage>) -> File {
        File {
            storage,
            stats: FileStats::default(),
        }
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
    Symlink(std::path::PathBuf),
}

pub struct Node {
    pub parent: usize,
    pub attr: FileAttr,
    pub item: NodeItem,
    pub effects: Group,
}
