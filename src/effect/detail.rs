use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    effect::{Effect, EffectResult},
    ftypes::ErrNo,
};

// Delay processing by X ms. {"duration_ms": 100}
#[derive(Serialize, Deserialize)]
pub struct Delay {
    duration_ms: u64,
}

impl Effect for Delay {
    fn apply(&self) -> EffectResult {
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
    fn apply(&self) -> EffectResult {
        let ret = |b| {
            if b {
                EffectResult::Error(self.errno)
            } else {
                EffectResult::Ack
            }
        };
        match self.cond {
            FlakeyCondition::Always { always } => ret(always),
            FlakeyCondition::Prob { prob } => ret(rand::rng().random::<f32>() <= prob),
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
