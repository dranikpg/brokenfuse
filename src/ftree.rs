use std::time::SystemTime;

use fuser::FileAttr;
use libc::ENOENT;

use crate::ftypes::{AttrOps, Dir, ErrNo, Ino, Node, NodeItem};

pub struct Tree {
    nodes: Vec<Option<Node>>,
    freelist: Vec<Ino>,
}

impl Tree {
    pub fn new<const N: usize>(nodes: [Node; N]) -> Tree {
        Tree {
            nodes: nodes.into_iter().map(|n| Some(n)).collect(),
            freelist: vec![],
        }
    }

    // Count number of occupied nodes
    pub fn count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn get(&self, ino: Ino) -> Option<&Node> {
        self.nodes.get(ino).map(&Option::as_ref).flatten()
    }

    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Node> {
        self.nodes.get_mut(ino).map(&Option::as_mut).flatten()
    }

    fn get_dir_mut(&mut self, ino: Ino) -> Option<(&mut Dir, &mut FileAttr)> {
        let node = self.get_mut(ino)?;
        match node.item {
            NodeItem::Dir(ref mut dir) => Some((dir, &mut node.attr)),
            _ => None,
        }
    }

    // Climb up from `ino` up to root yielding every node on the path
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
                // End node points to itself
                self.ino = Some(node.parent).take_if(|new_ino| *new_ino == ino);
                Some(node)
            }
        }
        It {
            ino: Some(ino),
            tree: &self,
        }
    }

    // Add node to `parent` under `name` pointing to `ino`
    fn add_entry(&mut self, ino: Ino, parent: Ino, name: String) -> Result<(), ErrNo> {
        let (pdir, pattr) = self.get_dir_mut(parent).ok_or(ENOENT)?;
        if pdir.lookup(&name).is_none() {
            pdir.add(ino, name);
            pattr.change_dir_balance(1);
            Ok(())
        } else {
            return Err(libc::EEXIST);
        }
    }

    // Remove entry from `parent` under `name` and return inode it was pointing to
    fn remove_entry(&mut self, parent: Ino, name: &str) -> Result<Ino, ErrNo> {
        let (pdir, pattr) = self.get_dir_mut(parent).ok_or(ENOENT)?;
        if let Some(ino) = pdir.lookup(name) {
            pdir.remove(name);
            pattr.change_dir_balance(-1);
            Ok(ino)
        } else {
            Err(ENOENT)
        }
    }

    // Create new entry at `parent`/`name` and return ino + reference to node slot
    pub fn create(&mut self, parent: Ino, name: String) -> Result<(Ino, &mut Option<Node>), ErrNo> {
        // Choose next ino ahead to avoid borrow (partial borrows where are youu...)
        let ino = self.freelist.pop().unwrap_or_else(|| {
            self.nodes.push(None);
            self.nodes.len() - 1
        });

        self.add_entry(ino, parent, name)
            .inspect_err(|_| self.freelist.push(ino))?;
        Ok((ino, &mut self.nodes[ino]))
    }

    // Create hard link
    pub fn link(&mut self, ino: Ino, parent: Ino, name: String) -> Result<FileAttr, ErrNo> {
        // Assert inode is valid
        if !self.nodes[ino].is_some() {
            return Err(ENOENT);
        }

        self.add_entry(ino, parent, name)?;

        let attr = &mut self.get_mut(ino).unwrap().attr;
        attr.change_nlink_balance(1);
        Ok(*attr)
    }

    pub fn rename(
        &mut self,
        old_parent: Ino,
        old_name: &str,
        parent: Ino,
        name: &str,
    ) -> Result<(), ErrNo> {
        let ino = self.remove_entry(old_parent, old_name)?;
        self.add_entry(ino, parent, name.to_owned())
            .inspect_err(|_| {
                // Restore previous state on insertion error
                self.add_entry(ino, old_parent, old_name.to_owned())
                    .unwrap()
            })
    }

    pub fn unlink(&mut self, parent: Ino, name: &str) -> Result<(), ErrNo> {
        let ino = self.remove_entry(parent, name)?;

        let attr = &mut self.get_mut(ino).unwrap().attr;
        attr.change_nlink_balance(-1);
        self.nodes[ino].take_if(|n| n.attr.nlink == 0);
        Ok(())
    }
}
