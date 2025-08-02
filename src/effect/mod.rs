use std::{collections::HashMap, ffi::os_str::Display, str::FromStr};

use crate::{
    effect::{self, detail::Delay},
    ftypes::ErrNo,
};
use bitflags::{Flags, bitflags};
use tinyjson::JsonValue;

mod detail;

pub trait Effect {
    fn apply(&self) -> Option<ErrNo>;
    fn serialize(&self) -> Vec<(&'static str, JsonValue)>;
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
        let parsed: JsonValue = data.parse().unwrap();
        let mut map: HashMap<_, _> = parsed.try_into().unwrap();
        let op: OpType = {
            let op_j = map.remove("op").unwrap();
            let op_str: String = op_j.try_into().unwrap();
            op_str.parse().unwrap()
        };
        let (sname, effect): (&'static str, Box<dyn Effect>) = match name {
            "delay" => ("delay", Box::new(Delay::new(map).unwrap())),
            _ => return Err(format!("")),
        };
        Ok(DefinedEffect {
            name: sname,
            effect,
            op,
        })
    }

    pub fn serialize(&self) -> JsonValue {
        let mut map: HashMap<String, JsonValue> = self
            .effect
            .serialize()
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v))
            .collect();
        map.insert("op".to_owned(), JsonValue::String(format!("{}", self.op)));
        JsonValue::Object(map)
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

    pub fn serialize(&self) -> JsonValue {
        let mut map: HashMap<String, JsonValue> = HashMap::new();
        for effect in self {
            map.insert(effect.name.to_owned(), effect.serialize());
        }
        JsonValue::Object(map)
    }
}
