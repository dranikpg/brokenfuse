use libc::ENOENT;
use std::io::empty;
use std::ops::Deref;
use std::rc::Rc;

use crate::effect::EffectGroup;
use crate::ftypes::{ErrNo, File, Ino, Node, NodeItem};

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

    // TODO: remove
    pub fn file_mut(&mut self, ino: Ino) -> Option<&mut File> {
        let node = self.nodes.get_mut(ino)?.as_mut()?;
        match node.item {
            NodeItem::File(ref mut file) => Some(file),
            _ => None,
        }
    }

    pub fn get(&self, ino: Ino) -> Option<&Node> {
        self.nodes.get(ino).map(&Option::as_ref).flatten()
    }

    pub fn get_mut(&mut self, ino: Ino) -> Option<&mut Node> {
        self.nodes.get_mut(ino).map(&Option::as_mut).flatten()
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.iter_mut().filter_map(|n| n.as_mut())
    }

    pub fn attach(&mut self, ino: Ino, effect: Rc<EffectGroup>) {
        let prev_effects = self.nodes[ino].as_mut().unwrap().effects.take();

        let mut stack = vec![ino];
        while let Some(ino) = stack.pop() {
            let node = self.nodes[ino].as_mut().unwrap();

            // Determine conditions for replacing node effect and searching further
            match (&node.effects, &prev_effects) {
                (None, _) => (),
                (Some(ref e1), Some(ref e2)) if Rc::ptr_eq(e1, e2) => (),
                (Some(_), None) => panic!("broken effect tree"),
                (Some(_), _) => continue, // unequal effects
            };
            
            println!("Attaching group to {}", node.attr.ino);
            node.effects.replace(effect.clone());
            if let NodeItem::Dir(ref dir) = node.item {
                stack.extend(dir.list().map(|(i, _)| i));
            }
        }
    }

    // Create entry for node and return reference to it
    pub fn create(
        &mut self,
        parent: Ino,
        name: String,
    ) -> Result<(Ino, Option<Rc<EffectGroup>>, &mut Option<Node>), ErrNo> {
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
                Ok((
                    ino,
                    self.nodes[parent].as_ref().unwrap().effects.clone(),
                    &mut self.nodes[ino],
                ))
            } else {
                Err(libc::EEXIST)
            }
        } else {
            Err(libc::ENOENT)
        }
    }

    // Erase node
    pub fn erase(&mut self, ino: Ino) -> bool {
        if let Some(node) = self.nodes[ino].take() {
            match self.nodes[node.parent].as_mut().unwrap().item {
                NodeItem::Dir(ref mut dir) => dir.remove(ino),
                _ => panic!("Corrupted tree: non-directory parent"),
            };
            true
        } else {
            false
        }
    }
}
