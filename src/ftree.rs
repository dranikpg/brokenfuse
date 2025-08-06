use std::{alloc::System, borrow::Cow, time::SystemTime};

use fuser::FileAttr;
use libc::ENOENT;

use crate::ftypes::{ErrNo, Ino, Node, NodeItem};

pub struct Tree {
    nodes: Vec<Option<Node>>,
    freelist: Vec<Ino>,
}

impl Tree {
    pub fn initial<const N: usize>(nodes: [Node; N]) -> Tree {
        Tree {
            nodes: nodes.into_iter().map(|n| Some(n)).collect(),
            freelist: vec![],
        }
    }

    pub fn count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn get(&self, ino: Ino) -> Option<&Node> {
        self.nodes.get(ino).map(&Option::as_ref).flatten()
    }

    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Node> {
        self.nodes.get_mut(ino).map(&Option::as_mut).flatten()
    }

    pub fn climb(&self, ino: Ino) -> impl Iterator<Item = &Node> {
        struct It<'a> {
            ino: Option<Ino>,
            tree: &'a Tree,
        }
        impl<'a> Iterator for It<'a> {
            type Item = &'a Node;

            fn next(&mut self) -> Option<Self::Item> {
                let ino = self.ino?;
                let node = self.tree.nodes[ino].as_ref()?;
                if node.parent != ino {
                    self.ino = Some(node.parent);
                } else {
                    self.ino = None;
                }
                Some(node)
            }
        }
        It {
            ino: Some(ino),
            tree: &self,
        }
    }

    // Create entry for node and return reference to it
    pub fn create(&mut self, parent: Ino, name: String) -> Result<(Ino, &mut Option<Node>), ErrNo> {
        // Assure freelist has at least one index
        if self.freelist.is_empty() {
            self.freelist.push(self.nodes.len());
            self.nodes.push(None);
        }

        let parent = self
            .nodes
            .get_mut(parent)
            .ok_or(ENOENT)?
            .as_mut()
            .ok_or(ENOENT)?;
        if let NodeItem::Dir(ref mut dir) = parent.item {
            if dir.lookup(&name).is_none() {
                let ino = self.freelist.pop().unwrap();
                dir.add(ino, name);
                parent.attr.mtime = SystemTime::now();
                parent.attr.ctime = SystemTime::now();
                parent.attr.blocks = 1;
                parent.attr.size += 1;
                Ok((ino, &mut self.nodes[ino]))
            } else {
                Err(libc::EEXIST)
            }
        } else {
            Err(libc::ENOENT)
        }
    }

    // Create hard link
    pub fn link(&mut self, ino: Ino, parent: Ino, name: String) -> Result<FileAttr, ErrNo> {
        if !self.nodes[ino].is_some() {
            return Err(ENOENT);
        }

        let parent = self
            .nodes
            .get_mut(parent)
            .ok_or(ENOENT)?
            .as_mut()
            .ok_or(ENOENT)?;
        if let NodeItem::Dir(ref mut dir) = parent.item {
            if dir.lookup(&name).is_none() {
                parent.attr.mtime = SystemTime::now();
                parent.attr.ctime = SystemTime::now();
                parent.attr.blocks = 1;
                parent.attr.size += 1;
                dir.add(ino, name);
            } else {
                return Err(libc::EEXIST);
            }
        } else {
            return Err(libc::ENOENT);
        }

        let node = self
            .nodes
            .get_mut(ino)
            .ok_or(ENOENT)?
            .as_mut()
            .ok_or(ENOENT)?;
        node.attr.nlink += 1;
        Ok(node.attr)
    }

    pub fn rename(
        &mut self,
        old_parent_ino: Ino,
        old_name: &str,
        parent_ino: Ino,
        name: &str,
    ) -> Option<Ino> {
        let old_parent = self.nodes[old_parent_ino].as_mut().unwrap();
        let ino = match old_parent.item {
            NodeItem::Dir(ref mut dir) => {
                old_parent.attr.mtime = SystemTime::now();
                old_parent.attr.ctime = SystemTime::now();
                old_parent.attr.size -= 1;
                dir.lookup(old_name).inspect(|_| dir.remove(old_name))
            }
            _ => panic!(""),
        };
        let parent = self.nodes[parent_ino].as_mut().unwrap();
        match parent.item {
            NodeItem::Dir(ref mut dir) => {
                parent.attr.mtime = SystemTime::now();
                parent.attr.ctime = SystemTime::now();
                parent.attr.blocks = 1;
                parent.attr.size += 1;
                dir.add(ino.unwrap(), name.to_owned());
            }
            _ => panic!(""),
        };
        ino
    }

    // Erase node
    pub fn unlink(&mut self, parent: Ino, name: &str) -> Option<()> {
        // Erase from parent
        let parent = self.nodes[parent].as_mut().unwrap();
        let ino = match parent.item {
            NodeItem::Dir(ref mut dir) => {
                parent.attr.mtime = SystemTime::now();
                parent.attr.ctime = SystemTime::now();
                parent.attr.size -= 1;
                dir.lookup(name).inspect(|_| dir.remove(name))?
            }
            _ => panic!("Corrupted tree: non-directory parent"),
        };

        let node = self.nodes[ino].as_mut().unwrap();
        node.attr.nlink -= 1;
        if node.attr.nlink == 0 {
            self.nodes[ino].take();
        }

        Some(())
    }
}
