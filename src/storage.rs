use crate::ftypes::Ino;
use std::{borrow::Cow, fs::File, os::unix::fs::FileExt};

pub trait Storage {
    fn len(&self) -> usize;
    fn read(&self, offset: usize, size: usize) -> Cow<'_, [u8]>;
    fn write(&mut self, offset: usize, data: &[u8]);
}

pub struct RamStorage {
    buffer: Vec<u8>,
}

impl RamStorage {
    pub fn create() -> RamStorage {
        RamStorage { buffer: vec![] }
    }
}

impl Storage for RamStorage {
    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn read(&self, offset: usize, size: usize) -> Cow<'_, [u8]> {
        let start = offset.min(self.buffer.len());
        let end = (offset + size).min(self.buffer.len());
        if start >= end {
            return Cow::from(vec![]);
        }
        Cow::from(&self.buffer[start..end])
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }
}

pub struct FileStorage {
    path: String,
    file: std::fs::File,
}

impl FileStorage {
    pub fn create(iden: &str) -> FileStorage {
        let path = format!("/tmp/bf-{}", iden);
        let file = File::options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .expect("Failed to open file");
        FileStorage { path, file }
    }
}

impl Drop for FileStorage {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

impl Storage for FileStorage {
    fn len(&self) -> usize {
        self.file.metadata().map(|m| m.len() as usize).unwrap_or(0)
    }

    fn read(&self, offset: usize, size: usize) -> Cow<'_, [u8]> {
        let mut buffer = vec![0; size];
        self.file.read_exact_at(buffer.as_mut(), offset as u64).ok();
        Cow::Owned(buffer)
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
        self.file.write_all_at(data, offset as u64).ok();
    }
}
