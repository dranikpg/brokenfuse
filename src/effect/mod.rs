use bitflags::bitflags;
use libc::EINVAL;
use serde::{Serialize, Serializer};
use serde_json::Value as JValue;
use std::any::Any;
use std::str::FromStr;
use std::time::Duration;

use crate::ftree;
use crate::ftypes::{ErrNo, Ino};
mod detail;

pub enum EffectResult {
    Ack,          // Acknowledge operation, don't do anything
    Error(ErrNo), // Cause error
    Delay(u64),   // Sleep ms
}

pub enum OpDesr {
    Read { offset: usize, len: usize },
    Write { offset: usize, len: usize },
}

impl OpDesr {
    fn optype(&self) -> OpType {
        match self {
            OpDesr::Read { .. } => OpType::R,
            OpDesr::Write { .. } => OpType::W,
        }
    }
}

pub struct Context<'a> {
    pub op: OpDesr,
    pub origin: Ino, // where the effect is defined at
    pub target: Ino, // where the effect is applied at
    pub tree: &'a ftree::Tree,
    pub rgen: &'a mut rand::rngs::StdRng,
}

pub trait Effect {
    fn apply(&self, ctx: &mut Context) -> EffectResult;
    fn as_any(&self) -> &dyn Any;
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
    type Err = ErrNo;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = OpType::empty();
        for c in s.chars() {
            let s: String = c.into();
            res |= OpType::from_name(&s.to_ascii_uppercase()).ok_or(EINVAL)?;
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

impl Serialize for OpType {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&format!("{}", self))
    }
}

#[derive(Serialize)]
pub struct DefinedEffect {
    pub name: String,
    #[serde(flatten, serialize_with = "serialize_box")]
    pub effect: Box<dyn Effect>,
    pub op: OpType,
}

fn serialize_box<S>(b: &Box<dyn Effect>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let a = b.as_any();
    if let Some(delay) = a.downcast_ref::<detail::Delay>() {
        delay.serialize(s)
    } else if let Some(flakey) = a.downcast_ref::<detail::Flakey>() {
        flakey.serialize(s)
    } else if let Some(maxsize) = a.downcast_ref::<detail::MaxSize>() {
        maxsize.serialize(s)
    } else {
        panic!("Unsupported dynamic type!");
    }
}

impl DefinedEffect {
    pub fn create(name: &str, data: &str) -> Result<Self, ErrNo> {
        let mut parsed: JValue = serde_json::from_str(data).unwrap();
        let op: OpType = parsed
            .as_object_mut()
            .and_then(|obj| obj.remove("op"))
            .and_then(|obj| obj.as_str().map(|s| s.to_owned()))
            .ok_or(EINVAL)?
            .parse()?;

        let (eftype, _) = name.split_once("-").unwrap_or((name, name));

        macro_rules! match_effect {
            ($($name:literal => $efft:ty),*) => {
                match eftype {
                    $($name => ($name, Box::new(serde_json::from_value::<$efft>(parsed).map_err(|_|EINVAL)?)),)*
                    _ => return Err(EINVAL),
                }
            };
        }

        let (sname, effect): (&'static str, Box<dyn Effect>) = match_effect! {
            "delay" => detail::Delay, "flakey" => detail::Flakey, "maxsize" => detail::MaxSize
        };
        Ok(DefinedEffect {
            name: sname.to_owned(),
            effect,
            op,
        })
    }
}

#[derive(Default, Serialize)]
pub struct Group {
    effects: Vec<DefinedEffect>,
}

impl<'a> IntoIterator for &'a Group {
    type Item = &'a DefinedEffect;
    type IntoIter = std::slice::Iter<'a, DefinedEffect>;
    fn into_iter(self) -> Self::IntoIter {
        self.effects.iter()
    }
}

impl Group {
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn remove(&mut self, name: &str) {
        self.effects.retain(|de| de.name != name);
    }

    pub fn add(&mut self, nde: DefinedEffect) {
        self.remove(&nde.name);
        self.effects.push(nde);
    }
}

pub fn run<'a>(
    it: impl Iterator<Item = &'a crate::ftypes::Node>,
    mut ctx: Context,
) -> (u64, Option<ErrNo>) {
    let mut sleep_ms: u64 = 0;
    let mut first_errno: Option<ErrNo> = None;
    'outer: for node in it {
        ctx.origin = node.attr.ino as Ino;
        for DefinedEffect { effect, op, .. } in &node.effects {
            if (ctx.op.optype() & *op).is_empty() {
                continue;
            }
            match effect.apply(&mut ctx) {
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
