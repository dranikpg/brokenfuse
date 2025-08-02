use clap::{Arg, Command};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};

mod effect;
mod ftree;
mod ftypes;
mod storage;
mod utility;

use ftree::Tree;
use ftypes::{Dir, ErrNo, File, Ino, Node, NodeItem};
use storage::{FileStorage, Storage};

use crate::effect::{DefinedEffect, EffectGroup, OpType};
use crate::storage::RamStorage;

const TTL: Duration = Duration::from_secs(1);

struct TestFS {
    tree: ftree::Tree,
}

// Create fresh attributes
fn fresh_attr(ino: Ino, kind: FileType, mode: u32, uid: u32, gid: u32) -> FileAttr {
    let now = SystemTime::now();
    FileAttr {
        ino: ino as u64,
        size: 0,
        blocks: 0,
        atime: now,
        mtime: now,
        ctime: now,
        crtime: now,
        kind,
        perm: mode as u16,
        nlink: 0,
        uid: uid,
        gid: gid,
        rdev: 0,
        blksize: 512,
        flags: 0,
    }
}

fn create_storage(ino: Ino) -> Box<dyn Storage> {
    //let fs = FileStorage::create(&format!("{}", ino));
    let fs = RamStorage::create();
    Box::new(fs)
}

impl TestFS {
    // Access generic node for reads
    fn access_node(&mut self, ino: Ino) -> Result<&Node, ErrNo> {
        self.tree
            .get_mut(ino)
            .map(|n| {
                n.attr.atime = SystemTime::now();
                n
            })
            .map(|n| &*n) // de-mut
            .ok_or(ENOENT)
    }

    // Access generic node for reads and writes
    fn access_node_mut(&mut self, ino: Ino) -> Result<&mut Node, ErrNo> {
        self.tree
            .get_mut(ino)
            .map(|n| {
                n.attr.atime = SystemTime::now();
                n.attr.mtime = SystemTime::now();
                n
            })
            .ok_or(ENOENT)
    }

    // Access directory for reads
    fn access_dir(&mut self, ino: Ino) -> Result<(&Dir, Ino), ErrNo> {
        let node = self.access_node(ino)?;
        if let NodeItem::Dir(ref dir) = node.item {
            Ok((dir, node.parent))
        } else {
            Err(ENOENT)
        }
    }

    fn create_node(
        &mut self,
        req: &Request,
        parent: Ino,
        name: &OsStr,
        mode: u32,
        kind: FileType,
    ) -> Result<FileAttr, ErrNo> {
        self.access_dir(parent)?; // Check parent folder for permissions

        let (ino, nref) = self
            .tree
            .create(parent, name.to_string_lossy().to_string())?;
        let item = match kind {
            FileType::Directory => NodeItem::Dir(Dir::default()),
            FileType::RegularFile => NodeItem::File(File::create(create_storage(ino))),
            _ => panic!("!"),
        };
        let attr = fresh_attr(ino, kind, mode, req.uid(), req.gid());
        let node = Node {
            parent,
            attr,
            item,
            effects: EffectGroup::default(),
        };
        nref.replace(node);
        Ok(attr)
    }

    fn erase_node(&mut self, parent: Ino, name: &OsStr) -> Result<Node, ErrNo> {
        let ino = self.access_dir(parent)?.0.lookup(name).ok_or(ENOENT)?;
        self.tree.erase(ino).ok_or(ENOENT)
    }

    fn play_effects(&self, ino: Ino, op_type: OpType) -> Option<ErrNo> {
        for node in self.tree.climb(ino) {
            for effect in &node.effects {
                if (effect.op & op_type).is_empty() {
                    continue;
                }

                if let Some(errno) = effect.effect.apply() {
                    return Some(errno);
                }
            }
        }
        None
    }
}

impl Filesystem for TestFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        match self
            .access_dir(parent as Ino)
            .and_then(|(d, _)| d.lookup(name).ok_or(ENOENT))
            .and_then(|ino| self.access_node(ino))
        {
            Ok(node) => reply.entry(&TTL, &node.attr, 0),
            Err(errno) => reply.error(errno),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        match self.access_node(ino as Ino) {
            Ok(node) => reply.attr(&TTL, &node.attr),
            Err(errno) => reply.error(errno),
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let node = match self.access_node_mut(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };

        if let Some(mode) = mode {
            node.attr.perm = mode as u16;
        }

        reply.attr(&TTL, &node.attr);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let (raw_entries, parent): (Vec<(Ino, String)>, Ino) = match self.access_dir(ino as Ino) {
            Ok((dir, parent)) => (dir.list().map(|(i, n)| (i, n.to_owned())).collect(), parent),
            Err(errno) => return reply.error(errno),
        };
        let base_entries = [
            (ino as usize, FileType::Directory, "."),
            (parent, FileType::Directory, ".."),
        ];
        let dir_entries = raw_entries.iter().map(|(fino, fname)| {
            (
                *fino,
                self.access_node(*fino).unwrap().attr.kind,
                fname.as_str(),
            )
        });
        for (i, e) in base_entries
            .into_iter()
            .chain(dir_entries)
            .enumerate()
            .skip(offset as usize)
        {
            if reply.add(e.0 as u64, (i + 1) as i64, e.1, e.2) {
                break;
            }
        }
        reply.ok();
    }

    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        match self.create_node(req, parent as Ino, name, mode, FileType::Directory) {
            Ok(attr) => reply.entry(&TTL, &attr, 0),
            Err(errno) => reply.error(errno),
        }
    }

    fn create(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        match self.create_node(req, parent as Ino, name, mode, FileType::RegularFile) {
            Ok(attr) => reply.created(&TTL, &attr, 0, attr.ino, 0),
            Err(errno) => reply.error(errno),
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: fuser::ReplyWrite,
    ) {
        let node = match self.access_node_mut(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };

        if let NodeItem::File(ref mut file) = node.item {
            file.storage_mut().write(offset as usize, data);
            node.attr.size = file.storage().len() as u64;
            node.attr.blocks = node.attr.size / (node.attr.blksize as u64) + 1;
        } else {
            return reply.error(ENOENT);
        }

        if let Some(errno) = self.play_effects(ino as Ino, OpType::R) {
            reply.error(errno);
        } else {
            reply.written(data.len() as u32);
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let node = match self.access_node(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };

        let data = if let NodeItem::File(ref file) = node.item {
            file.storage()
                .read(offset as usize, size as usize)
                .into_owned()
        } else {
            return reply.error(ENOENT);
        };

        if let Some(errno) = self.play_effects(ino as Ino, OpType::R) {
            reply.error(errno);
        } else {
            reply.data(&data);
        }
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: fuser::ReplyEmpty,
    ) {
        let mut node = match self.erase_node(parent as Ino, name) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };
        let (ino, nref) = match self
            .tree
            .create(newparent as Ino, newname.to_string_lossy().to_string())
        {
            Ok(t) => t,
            Err(errno) => return reply.error(errno),
        };

        node.parent = newparent as Ino;
        node.attr.ino = ino as u64;
        nref.replace(node);
        reply.ok();
    }

    fn flush(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: fuser::ReplyEmpty,
    ) {
        reply.ok();
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        match self.erase_node(parent as Ino, name) {
            Ok(_) => reply.ok(),
            Err(errno) => reply.error(errno),
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        match self.erase_node(parent as Ino, name) {
            Ok(_) => reply.ok(),
            Err(errno) => reply.error(errno),
        }
    }

    fn getxattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        size: u32,
        reply: fuser::ReplyXattr,
    ) {
        let name = name.to_string_lossy();
        if name == "bk.effect" {
            let node = self.tree.get_mut(ino as Ino).unwrap();
            let out = node.effects.serialize().stringify().unwrap();
            if size == 0 {
                reply.size(out.as_bytes().len() as u32);
            } else {
                reply.data(out.as_bytes())
            } // todo size check
        } else {
            reply.error(ENOENT);
        }
    }

    fn setxattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        value: &[u8],
        _flags: i32,
        _position: u32,
        reply: fuser::ReplyEmpty,
    ) {
        let name = name.to_string_lossy();
        if let Some(ename) = name.strip_prefix("bk.effect.") {
            let effect = DefinedEffect::create(ename, &String::from_utf8_lossy(value)).unwrap();
            self.tree.get_mut(ino as Ino).unwrap().effects.add(effect);
        }

        println!("Set xattr {} {}", name, String::from_utf8_lossy(value));
        reply.ok();
    }

    fn removexattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        let name = name.to_string_lossy();
        if name == "bk.effect" {
            self.tree.get_mut(ino as Ino).unwrap().effects.clear();
        }

        if let Some(ename) = name.strip_prefix("bk.effect.") {
            self.tree.get_mut(ino as Ino).unwrap().effects.remove(ename);
        }

        reply.ok();
    }
}

fn main() {
    let matches = Command::new("brokenfuse")
        .version("1.0")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Act as a client, and mount FUSE at given path"),
        )
        .get_matches();
    env_logger::init();

    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let options = vec![
        MountOption::RW,
        MountOption::FSName("hello".to_string()),
        MountOption::DefaultPermissions,
        MountOption::AutoUnmount,
        MountOption::AllowRoot,
    ];

    let nodes = [
        Node {
            parent: 0,
            item: NodeItem::Dir(Dir::default()),
            attr: fresh_attr(0, FileType::Directory, 0x000, 1000, 1001),
            effects: EffectGroup::default(),
        },
        Node {
            parent: 1,
            item: NodeItem::Dir(Dir::default()),
            attr: fresh_attr(1, FileType::Directory, 0o754, 1000, 1001),
            effects: EffectGroup::default(),
        },
    ];
    let mut tree = Tree::initial(nodes);
    //tree.attach(
    //    1,
    //    Some(EffectGroup::new(
    //        None,
    //        [Box::new(effect::Delay {})].map(|b| (OpType::all(), b as Box<dyn Effect>)),
    //    )),
    //);
    fuser::mount2(TestFS { tree }, mountpoint, &options).unwrap();

    println!()
}
