use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    effect::{Context, Effect, EffectResult, OpDesr},
    ftypes::ErrNo,
};

// Delay processing by X ms. {"duration_ms": 100}
#[derive(Serialize, Deserialize)]
pub struct Delay {
    duration_ms: u64,
}

impl Effect for Delay {
    fn apply(&self, _ctx: &mut Context) -> EffectResult {
        EffectResult::Delay(self.duration_ms)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FlakeyCondition {
    Always { always: bool },
    Prob { prob: f32 },
    Interval { avail: u64, unavail: u64 },
}

// Return `errno` (EIO by default) with:
// 1. Always or never {"always": true/false }
// 2. `prob`% probability {"prob": 0.3, "errno": 5}
// 3. `avail`/`unavail` intervals in milliseconds {"avail": 5, "unavail": 10}
#[derive(Serialize, Deserialize)]
pub struct Flakey {
    #[serde(flatten)]
    cond: FlakeyCondition,
    #[serde(default = "Flakey::default_errno")]
    errno: libc::c_int,
}

impl Flakey {
    fn default_errno() -> ErrNo {
        libc::EIO
    }
}

impl Effect for Flakey {
    fn apply(&self, ctx: &mut Context) -> EffectResult {
        let ret = |b| {
            if b {
                EffectResult::Error(self.errno)
            } else {
                EffectResult::Ack
            }
        };
        match self.cond {
            FlakeyCondition::Always { always } => ret(always),
            FlakeyCondition::Prob { prob } => ret(ctx.rgen.random::<f32>() <= prob),
            FlakeyCondition::Interval { avail, unavail } => {
                let passed_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();

                let rem = (passed_ms) % ((avail + unavail) as u128);
                ret(rem <= avail as u128)
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaxSize {
    limit: usize,
}

impl Effect for MaxSize {
    fn apply(&self, ctx: &mut Context) -> EffectResult {
        let (offset, len) = match &ctx.op {
            OpDesr::Write { offset, len } => (offset, len),
            _ => return EffectResult::Ack,
        };

        // Determine by how much file would need to grow
        let file_size = ctx.tree.get(ctx.target).unwrap().attr.size;
        let need_grow = (offset + len) as i64 - file_size as i64;
        if need_grow < 0 {
            return EffectResult::Ack;
        }

        // Determine subtree size
        let total_size = ctx
            .tree
            .traverse(ctx.origin)
            .filter(|n| n.attr.kind == fuser::FileType::RegularFile)
            .map(|n| n.attr.size as i64)
            .sum::<i64>();

        if total_size + need_grow > self.limit as i64 {
            EffectResult::Error(libc::ENOSPC)
        } else {
            EffectResult::Ack
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }
}
