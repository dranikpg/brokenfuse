use std::time::SystemTime;

use fuser::FileAttr;

pub trait ImmutCounter {
    fn add(&self, u: impl TryInto<usize>);
    fn incr(&self) {
        self.add(1usize);
    }
}

// Treat Cell<usize> as immutable counter
impl ImmutCounter for std::cell::Cell<usize> {
    fn add(&self, u: impl TryInto<usize>) {
        self.update(|v| v + (u.try_into().unwrap_or(0)));
    }
}

pub trait AttrOps {
    fn dir_balance(&mut self, balance: i8);
    fn nlink_balance(&mut self, balance: i8);
}

impl AttrOps for FileAttr {
    fn dir_balance(&mut self, balance: i8) {
        self.mtime = SystemTime::now();
        self.ctime = self.mtime;
        self.size = self.size.wrapping_add_signed(balance as i64);
        self.blocks = self.size / self.blksize as u64;
    }

    fn nlink_balance(&mut self, balance: i8) {
        self.ctime = SystemTime::now();
        self.nlink = self.nlink.wrapping_add_signed(balance as i32);
    }
}
