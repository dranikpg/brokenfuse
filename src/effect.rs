use crate::ftypes::ErrNo;
use std::rc::Rc;

pub trait Effect {
    fn apply(&self) -> Option<ErrNo>;
}

pub struct EffectGroup {
    list: Vec<Box<dyn Effect>>,
    prev: Option<Rc<EffectGroup>>,
}

impl EffectGroup {
    pub fn new(
        prev: Option<Rc<EffectGroup>>,
        effects: impl IntoIterator<Item = Box<dyn Effect>>,
    ) -> Rc<Self> {
        Rc::new(Self {
            list: effects.into_iter().collect(),
            prev,
        })
    }

    pub fn list(&self) -> impl Iterator<Item = &dyn Effect> {
        self.list.iter().map(|b| b.as_ref())
    }

    pub fn climb(cur: &Option<Rc<EffectGroup>>) -> impl Iterator<Item = Rc<EffectGroup>> {
        struct It(Option<Rc<EffectGroup>>);
        impl Iterator for It {
            type Item = Rc<EffectGroup>;
            fn next(&mut self) -> Option<Self::Item> {
                let next = self.0.clone().map(|g| g.prev.clone()).flatten();
                std::mem::replace(&mut self.0, next)
            }
        }
        It(cur.clone())
    }
}

struct AlwaysErr {}

pub struct Delay {}

impl Effect for Delay {
    fn apply(&self) -> Option<ErrNo> {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        None
    }
}
