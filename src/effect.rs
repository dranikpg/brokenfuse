use bitflags::bitflags;
use std::{cell::RefCell, rc::Rc};

use crate::ftypes::ErrNo;

pub trait Effect {
    fn apply(&self) -> Option<ErrNo>;
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct OpType : u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const L = 1 << 2;
        const M = 1 << 3;
    }
}

pub struct EffectGroup {
    list: Vec<(OpType, Box<dyn Effect>)>,
    prev: RefCell<Option<Rc<EffectGroup>>>,
}

impl EffectGroup {
    pub fn new(
        prev: Option<Rc<EffectGroup>>,
        effects: impl IntoIterator<Item = (OpType, Box<dyn Effect>)>,
    ) -> Rc<Self> {
        Rc::new(Self {
            list: effects.into_iter().collect(),
            prev: RefCell::from(prev),
        })
    }

    pub fn list(&self) -> impl Iterator<Item = (OpType, &dyn Effect)> {
        self.list.iter().map(|(t, b)| (*t, b.as_ref()))
    }

    pub fn climb(cur: &Option<Rc<EffectGroup>>) -> impl Iterator<Item = Rc<EffectGroup>> {
        struct It(Option<Rc<EffectGroup>>);
        impl Iterator for It {
            type Item = Rc<EffectGroup>;
            fn next(&mut self) -> Option<Self::Item> {
                let next = match &self.0 {
                    Some(node) => node.prev.borrow().clone(),
                    None => None,
                };
                std::mem::replace(&mut self.0, next)
            }
        }
        It(cur.clone())
    }

    pub fn rewire(&self, prev: Option<Rc<EffectGroup>>) {
        self.prev.replace(prev);
    }
}

pub struct Delay {}

impl Effect for Delay {
    fn apply(&self) -> Option<ErrNo> {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        None
    }
}
