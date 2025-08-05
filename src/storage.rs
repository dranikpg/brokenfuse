use std::{
    borrow::Cow,
    fs::File,
    os::unix::fs::FileExt,
    path::{Path, PathBuf}, str::FromStr,
};

use crate::ftypes::Ino;

pub trait Storage {
    fn len(&self) -> usize;
    fn read(&self, offset: usize, size: usize) -> Cow<'_, [u8]>;
    fn write(&mut self, offset: usize, data: &[u8]);
}

pub struct Stat {
    pub blocks: u64,
    pub bavail: u64,
}

pub trait Factory {
    fn create(&self, ino: Ino) -> Box<dyn Storage>;
    fn statfs(&self) -> Stat;
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
        let end = offset + data.len();
        if end >= self.buffer.len() {
            self.buffer.resize(end, 0);
        }
        let dest: &mut [u8] = &mut self.buffer[offset..offset + data.len()];
        dest.copy_from_slice(data);
    }
}

pub struct RamSFactory;

impl Factory for RamSFactory {
    fn create(&self, _ino: Ino) -> Box<dyn Storage> {
        Box::new(RamStorage::create())
    }

    fn statfs(&self) -> Stat {
        let mi = meminfo::MemInfo::new().unwrap();
        let mut values = mi.parse();
        let total = values.next().unwrap().size().unwrap() * 1024;
        let available = values.skip(1).next().unwrap().size().unwrap() * 1024;
        Stat {
            blocks: (total / 4096) as u64,
            bavail: (available / 4096) as u64,
        }
    }
}

pub struct FileStorage {
    path: PathBuf,
    file: std::fs::File,
}

impl FileStorage {
    #[allow(dead_code)]
    pub fn create(path: &Path) -> FileStorage {
        let file = File::options()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)
            .expect("Failed to open file");
        FileStorage {
            path: path.to_owned(),
            file,
        }
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

pub struct FileSFactory {
    basepath: PathBuf,
}

impl Factory for FileSFactory {
    fn create(&self, ino: Ino) -> Box<dyn Storage> {
        let path = self.basepath.join(&format!("file-{}", ino));
        Box::new(FileStorage::create(&path))
    }

    fn statfs(&self) -> Stat {
        Stat {
            blocks: 100,
            bavail: 100,
        }
    }
}

impl FileSFactory {
    pub fn new(path: &str) -> Self {
        FileSFactory { basepath: PathBuf::from_str(path).unwrap() }
    }
}
