use fuser::FileAttr;
use serde::Serialize;
use std::cell::Cell;

use crate::effect::EffectGroup;
use crate::storage::Storage;
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
    pub fn remove(&mut self, name: &(impl PartialEq<str> + ?Sized)) {
        self.children.retain(|(_, fname)| name != fname.as_str());
    }
}

#[derive(Default)]
pub struct ImmutCounter(Cell<usize>);

impl ImmutCounter {
    pub fn record(&self, u: impl TryInto<usize>) {
        self.0.update(|v| v + (u.try_into().unwrap_or(0)));
    }
    pub fn incr(&self) {
        self.record(1usize);
    }
}

impl Serialize for ImmutCounter {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.0.get() as u64)
    }
}

#[derive(Default, Serialize)]
pub struct FileStats {
    pub reads: ImmutCounter,
    pub read_volume: ImmutCounter,
    pub writes: ImmutCounter,
    pub write_volume: ImmutCounter,
    pub errors: ImmutCounter,
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
    pub effects: EffectGroup,
}
