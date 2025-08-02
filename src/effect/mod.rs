use bitflags::bitflags;
use serde_json::Value as JValue;
use std::str::FromStr;
use std::time::Duration;

use crate::ftree::Tree;
use crate::ftypes::{ErrNo, Ino};
mod detail;

pub enum EffectResult {
    Ack,          // Acknowledge operation, don't do anything
    Error(ErrNo), // Cause error
    Delay(u64),   // Sleep ms
}

pub trait Effect {
    fn apply(&self) -> EffectResult;
    fn serialize(&self) -> serde_json::Value;
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

impl FromStr for OpType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = OpType::empty();
        for c in s.chars() {
            let s: String = c.into();
            res |= OpType::from_name(&s.to_ascii_uppercase()).ok_or(())?;
        }
        Ok(res)
    }
}

impl std::fmt::Display for OpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<String> = self.iter_names().map(|(n, _)| n.to_owned()).collect();
        f.write_str(&names.join(""))
    }
}

pub struct DefinedEffect {
    pub name: &'static str,
    pub effect: Box<dyn Effect>,
    pub op: OpType,
}

impl DefinedEffect {
    pub fn create(name: &str, data: &str) -> Result<Self, String> {
        let mut parsed: JValue = serde_json::from_str(data).unwrap();
        let op: OpType = {
            let op_str = parsed
                .as_object_mut()
                .unwrap()
                .remove("op")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            op_str.parse().unwrap()
        };

        macro_rules! match_effect {
            ($($name:literal => $efft:ty),*) => {
                match name {
                    $($name => {
                        let pt: $efft = serde_json::from_value(parsed).unwrap();
                        ($name, Box::new(pt))
                    },)*
                    _ => return Err(format!("")),
                }
            };
        }

        let (sname, effect): (&'static str, Box<dyn Effect>) = match_effect! {
            "delay" => detail::Delay, "flakey" => detail::Flakey
        };
        Ok(DefinedEffect {
            name: sname,
            effect,
            op,
        })
    }

    pub fn serialize(&self) -> JValue {
        let mut map = match self.effect.serialize() {
            JValue::Object(obj) => obj,
            _ => panic!("bad serialization"),
        };
        map.insert("op".to_owned(), JValue::String(format!("{}", self.op)));
        JValue::Object(map)
    }
}

#[derive(Default)]
pub struct EffectGroup {
    effects: Vec<DefinedEffect>,
}

impl<'a> IntoIterator for &'a EffectGroup {
    type Item = &'a DefinedEffect;
    type IntoIter = std::slice::Iter<'a, DefinedEffect>;
    fn into_iter(self) -> Self::IntoIter {
        self.effects.iter()
    }
}

impl EffectGroup {
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn remove(&mut self, name: &str) {
        self.effects.retain(|de| de.name != name);
    }

    pub fn add(&mut self, nde: DefinedEffect) {
        self.remove(nde.name);
        self.effects.push(nde);
    }

    pub fn serialize(&self) -> JValue {
        let mut list: Vec<JValue> = vec![];
        for effect in self {
            let mut map = serde_json::Map::<String, JValue>::new();
            map.insert("name".to_owned(), JValue::String(effect.name.to_owned()));
            map.insert("effect".to_owned(), effect.serialize());
            list.push(JValue::Object(map));
        }
        JValue::Array(list)
    }
}

pub fn run(tree: &Tree, ino: Ino, fop: OpType) -> (u64, Option<ErrNo>) {
    let mut sleep_ms: u64 = 0;
    let mut first_errno: Option<ErrNo> = None;
    'outer: for node in tree.climb(ino) {
        for DefinedEffect { effect, op, .. } in &node.effects {
            if (fop & *op).is_empty() {
                continue;
            }
            match effect.apply() {
                EffectResult::Ack => (),
                EffectResult::Error(errno) => {
                    first_errno = Some(errno);
                    break 'outer;
                }
                EffectResult::Delay(ms) => {
                    sleep_ms += ms;
                }
            }
        }
    }
    (sleep_ms, first_errno)
}

// Reply, possibly delayed
pub fn reply(sleep_ms: u64, replier: impl FnOnce() + Send + 'static) {
    if sleep_ms >= 5 {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(sleep_ms));
            replier();
        });
    } else {
        if sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
        replier()
    }
}
