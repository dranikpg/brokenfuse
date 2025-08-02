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

    pub fn get(&self, ino: Ino) -> Option<&Node> {
        self.nodes.get(ino).map(&Option::as_ref).flatten()
    }

    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Node> {
        self.nodes.get_mut(ino).map(&Option::as_mut).flatten()
    }

    pub fn climb(&self, ino: Ino) -> impl Iterator<Item = &Node> {
        struct It<'a> {
            ino: Option<Ino>,
            tree: &'a Tree
        }
        impl <'a> Iterator for It<'a> {
            type Item = &'a Node;
        
            fn next(&mut self) -> Option<Self::Item> {
                let ino = self.ino?;
                let node = self.tree.nodes[ino].as_ref()?;
                if node.parent != ino {
                    self.ino = Some(node.parent);
                } else  {
                    self.ino = None;
                }
                Some(node)
            }
        }
        It{ino: Some(ino), tree: &self}
    }

    // Create entry for node and return reference to it
    pub fn create(&mut self, parent: Ino, name: String) -> Result<(Ino, &mut Option<Node>), ErrNo> {
        // Assure freelist has at least one index
        if self.freelist.is_empty() {
            self.freelist.push(self.nodes.len());
            self.nodes.push(None);
        }

        if let NodeItem::Dir(ref mut dir) = self
            .nodes
            .get_mut(parent)
            .ok_or(ENOENT)?
            .as_mut()
            .ok_or(ENOENT)?
            .item
        {
            if dir.lookup(&name).is_none() {
                let ino = self.freelist.pop().unwrap();
                dir.add(ino, name);
                Ok((ino, &mut self.nodes[ino]))
            } else {
                Err(libc::EEXIST)
            }
        } else {
            Err(libc::ENOENT)
        }
    }

    // Erase node
    pub fn erase(&mut self, ino: Ino) -> Option<Node> {
        let node = self.nodes[ino].take()?;

        // Erase from parent
        let parent = self.nodes[node.parent].as_mut().unwrap();
        match parent.item {
            NodeItem::Dir(ref mut dir) => dir.remove(ino),
            _ => panic!("Corrupted tree: non-directory parent"),
        };

        Some(node)
    }
}
