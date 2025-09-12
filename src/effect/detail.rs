use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections,
    time::{SystemTime, UNIX_EPOCH},
    usize,
};

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
    Prob { prob: f32 },
    Interval { avail_ms: u64, unavail_ms: u64 },
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
            FlakeyCondition::Prob { prob } => ret(ctx.rgen.random::<f32>() <= prob),
            FlakeyCondition::Interval { avail_ms, unavail_ms } => {
                let passed_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();

                let rem = (passed_ms) % ((avail_ms + unavail_ms) as u128);
                ret(rem <= avail_ms as u128)
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

// Build heatmap for given file
#[derive(Serialize, Deserialize)]
pub struct HeatMap {
    align: usize,
    #[serde(skip)]
    values: std::cell::RefCell<
        std::collections::BTreeMap<(usize /* offset */, usize /* len */), usize>,
    >,
}

impl Effect for HeatMap {
    fn apply(&self, ctx: &mut Context) -> EffectResult {
        let (mut offset, mut len) = match &ctx.op {
            OpDesr::Write { offset, len } => (*offset, *len),
            OpDesr::Read { offset, len } => (*offset, *len),
            _ => return EffectResult::Ack,
        };

        // Crop parameters
        let file_size = ctx.tree.get(ctx.target).unwrap().attr.size;

        offset = offset.min(file_size as usize);
        len = len.min(file_size as usize - offset);
        offset = offset / self.align * self.align;
        len = (len + self.align - 1) / self.align * self.align;

        self.values
            .borrow_mut()
            .entry((offset, len))
            .and_modify(|e| *e += 1)
            .or_insert(1);

        EffectResult::Ack
    }

    fn as_any(&self) -> &dyn std::any::Any {
        return self;
    }

    fn display(&self) -> Option<String> {
        let values = self.values.borrow();
        let mut out: Vec<(usize, usize)> = vec![];
        let mut record = |offset, balance| {
            if let Some((last, last_balance)) = out.last_mut()
                && *last == offset
            {
                *last_balance = balance;
            } else {
                out.push((offset, balance));
            }
        };

        let mut removals = collections::BTreeSet::<(usize, usize)>::new();

        let mut balance: usize = 0;
        for (start, add_delta) in values.iter() {
            // Subtract all operations that end at this index
            while let Some((off, rem_delta)) = removals.first()
                && *off <= start.0
            {
                balance -= rem_delta;
                record(*off, balance);
                removals.pop_first();
            }

            // Apply current operation and add it to removals
            balance += *add_delta;
            removals.insert((start.0 + start.1, *add_delta));
            record(start.0, balance);
        }

        while let Some((off, rem_delta)) = removals.pop_first() {
            balance -= rem_delta;
            record(off, balance);
        }

        Some(serde_json::to_string(&out).unwrap())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Quota {
    volume: usize,
    align: usize,
    #[serde(skip)]
    current: std::cell::Cell<usize>
}

impl Effect for Quota {
    fn apply(&self, ctx: &mut Context) -> EffectResult {
         let (_, mut len) = match &ctx.op {
            OpDesr::Write { offset, len } => (*offset, *len),
            OpDesr::Read { offset, len } => (*offset, *len),
            _ => return EffectResult::Ack,
        };
        len = (len + self.align - 1) / self.align * self.align;
        self.current.update(|v| v + len);

        if self.current.get() < self.volume {
            EffectResult::Ack
        } else {
            EffectResult::Error(libc::EDQUOT)
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        return  self;
    }
}
