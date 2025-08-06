use clap::Parser;
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request, TimeOrNow,
};
use libc::ENOENT;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, SystemTime};

mod effect;
mod ftree;
mod ftypes;
mod storage;
mod xaops;

use ftree::Tree;
use ftypes::{Dir, ErrNo, File, Ino, Node, NodeItem};

use crate::effect::{EffectGroup, OpType};

const TTL: Duration = Duration::from_secs(1);

struct TestFS {
    tree: ftree::Tree,
    sfactory: Box<dyn storage::Factory>,
}

enum NodeCreateT<'a> {
    Dir,
    File,
    Symlink(&'a std::path::Path),
}
struct NodeCreateReq<'a> {
    ntype: NodeCreateT<'a>,
    req: &'a Request<'a>,
}

// Create fresh attributes
fn fresh_attr(ino: Ino, kind: FileType, flags: u32, mode: u32, uid: u32, gid: u32) -> FileAttr {
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
        nlink: 1,
        uid: uid,
        gid: gid,
        rdev: 0,
        blksize: 4096,
        flags,
    }
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
        NodeCreateReq { req, ntype }: NodeCreateReq,
        parent: Ino,
        name: &OsStr,
        mode: u32,
        flags: u32,
    ) -> Result<FileAttr, ErrNo> {
        let (ino, nref) = self
            .tree
            .create(parent, name.to_string_lossy().to_string())?;

        let (kind, item) = match ntype {
            NodeCreateT::Dir => (FileType::Directory, NodeItem::Dir(Dir::default())),
            NodeCreateT::File => {
                let storage = self.sfactory.create(ino);
                (FileType::RegularFile, NodeItem::File(File::create(storage)))
            }
            NodeCreateT::Symlink(path) => (FileType::Symlink, NodeItem::Symlink(path.to_owned())),
            _ => panic!("!"),
        };

        let attr = fresh_attr(ino, kind, flags, mode, req.uid(), req.gid());
        let node = Node {
            parent,
            attr,
            item,
            effects: EffectGroup::default(),
        };
        nref.replace(node);
        Ok(attr)
    }

    fn unlink(&mut self, parent: Ino, name: &OsStr) -> Result<(), ErrNo> {
        self.tree
            .unlink(parent, &name.to_string_lossy())
            .ok_or(ENOENT)
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
        size: Option<u64>,
        atime: Option<fuser::TimeOrNow>,
        mtime: Option<fuser::TimeOrNow>,
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

        if let Some(size) = size {
            match node.item {
                NodeItem::File(ref mut f) => {
                    f.storage_mut().truncate(size as usize);
                    node.attr.size = size;
                    node.attr.blocks = size / node.attr.blksize as u64;
                }, 
                _ => panic!("")
            }
        }

        let tontot = |ton: TimeOrNow| match ton {
            TimeOrNow::Now => SystemTime::now(),
            TimeOrNow::SpecificTime(time) => time
        };

        if let Some(atime) = atime {
            node.attr.atime = tontot(atime);
        }

        if let Some(mtime) = mtime {
            node.attr.mtime = tontot(mtime);
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
        let req = NodeCreateReq {
            req,
            ntype: NodeCreateT::Dir,
        };
        match self.create_node(req, parent as Ino, name, mode, 0) {
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
        flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        match self.create_node(
            NodeCreateReq {
                ntype: NodeCreateT::File,
                req,
            },
            parent as Ino,
            name,
            mode,
            flags as u32,
        ) {
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
        let (ef_sleep, ef_err) = effect::run(&self.tree, ino as Ino, OpType::W);
        if let Some(errno) = ef_err {
            effect::reply(ef_sleep, move || reply.error(errno));
            return;
        }

        let node = match self.access_node_mut(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };

        let written = if let NodeItem::File(ref mut file) = node.item {
            file.storage_mut().write(offset as usize, data);
            node.attr.size = file.storage().len() as u64;
            node.attr.blocks = (node.attr.size / (node.attr.blksize as u64)) + 1;

            file.stats.writes.incr();
            file.stats.write_volume.record(data.len());
            Some(data.len())
        } else {
            None
        };

        effect::reply(ef_sleep, move || {
            if let Some(written) = written {
                reply.written(written as u32);
            } else {
                reply.error(ENOENT)
            }
        });
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
        let (ef_sleep, ef_err) = effect::run(&self.tree, ino as Ino, OpType::R);
        if let Some(errno) = ef_err {
            effect::reply(ef_sleep, move || reply.error(errno));
            return;
        }

        let node = match self.access_node(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };

        let data = if let NodeItem::File(ref file) = node.item {
            let data = file
                .storage()
                .read(offset as usize, size as usize)
                .into_owned();
            file.stats.reads.incr();
            file.stats.read_volume.record(data.len());
            Some(data)
        } else {
            None
        };

        effect::reply(ef_sleep, move || {
            if let Some(data) = data {
                reply.data(&data)
            } else {
                reply.error(ENOENT)
            }
        });
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
        let ino = match self.tree.rename(
            parent as Ino,
            name.to_string_lossy().as_ref(),
            newparent as Ino,
            newname.to_string_lossy().as_ref(),
        ) {
            Some(ino) => ino,
            None => return reply.error(ENOENT),
        };
        let node = self.access_node_mut(ino).unwrap();
        node.parent = newparent as Ino;
        node.attr.ctime = SystemTime::now();
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
        match self.unlink(parent as Ino, name) {
            Ok(_) => reply.ok(),
            Err(errno) => reply.error(errno),
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        match self.unlink(parent as Ino, name) {
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
        match xaops::get(&self.tree, ino as Ino, &name.to_string_lossy()) {
            Some(v) if size as usize > v.as_bytes().len() => reply.data(v.as_bytes()),
            Some(v) => reply.size(v.as_bytes().len() as u32),
            None => reply.error(ENOENT),
        };
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
        match xaops::set(
            &mut self.tree,
            ino as Ino,
            &name.to_string_lossy(),
            &String::from_utf8_lossy(value),
        ) {
            Some(_) => reply.ok(),
            None => reply.error(ENOENT),
        }
    }

    fn removexattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        name: &OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        match xaops::remove(&mut self.tree, ino as Ino, &name.to_string_lossy()) {
            Some(_) => reply.ok(),
            None => reply.error(ENOENT),
        }
    }

    fn fallocate(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        length: i64,
        mode: i32,
        reply: fuser::ReplyEmpty,
    ) {
        reply.ok();
    }

    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: fuser::ReplyStatfs) {
        let storage::Stat { blocks, bavail } = self.sfactory.statfs();
        reply.statfs(
            blocks,
            bavail,
            bavail,
            self.tree.count() as u64,
            100500,
            4096,
            255,
            0,
        );
    }

    fn symlink(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        link_name: &OsStr,
        target: &std::path::Path,
        reply: ReplyEntry,
    ) {
        let req = NodeCreateReq {
            ntype: NodeCreateT::Symlink(target),
            req,
        };
        match self.create_node(req, parent as Ino, link_name, 0x777, 0) {
            Ok(ref attr) => reply.entry(&TTL, attr, 0),
            Err(errno) => reply.error(errno),
        }
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        let node = match self.access_node(ino as Ino) {
            Ok(node) => node,
            Err(errno) => return reply.error(errno),
        };
        if let NodeItem::Symlink(ref path) = node.item {
            reply.data(&path.as_os_str().as_bytes());
        } else {
            reply.error(ENOENT);
        }
    }

    fn link(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        newparent: u64,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        match self.tree.link(
            ino as Ino,
            newparent as Ino,
            newname.to_string_lossy().to_string(),
        ) {
            Ok(ref attr) => reply.entry(&TTL, attr, 0),
            Err(errno) => reply.error(errno),
        }
    }
}

// Broken fuse FS
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Mount point of filesystem
    #[arg(value_name = "MOUNT_POINT", index = 1)]
    mount_path: String,

    // Pass through file storage
    #[arg(short, long)]
    passthrough: Option<String>,
}

fn main() {
    let args = Args::parse();
    env_logger::init();

    let mountpoint = args.mount_path;
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
            attr: fresh_attr(0, FileType::Directory, 0, 0x000, 1000, 1001),
            effects: EffectGroup::default(),
        },
        Node {
            parent: 1,
            item: NodeItem::Dir(Dir::default()),
            attr: fresh_attr(1, FileType::Directory, 0, 0o754, 1000, 1001),
            effects: EffectGroup::default(),
        },
    ];
    let tree = Tree::initial(nodes);
    let sfactory = if let Some(path) = args.passthrough {
        Box::new(storage::FileSFactory::new(&path)) as Box<dyn storage::Factory>
    } else {
        Box::new(storage::RamSFactory)
    };
    fuser::mount2(TestFS { tree, sfactory }, mountpoint, &options).unwrap();
}
