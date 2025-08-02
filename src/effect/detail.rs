use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{effect::Effect, ftypes::ErrNo};

// Delay processing by X ms. {"millis": 100}
#[derive(Serialize, Deserialize)]
pub struct Delay {
    #[serde(rename = "millis")]
    duration_ms: u64,
}

impl Effect for Delay {
    fn apply(&self) -> Option<ErrNo> {
        println!("Duration millis {}", self.duration_ms);
        std::thread::sleep(Duration::from_millis(self.duration_ms));
        println!("done");
        None
    }

    fn serialize(&self) -> serde_json::Value {
        serde_json::to_value(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FlakeyCondition {
    Always{},
    Prob { prob: f32 },
    Interval { avail: f64, unavail: f64 },
}

// Return `errno` (EIO by default) with:
// 1. `prob`% probability. {"prob": 0.3, "errno": 5}
// 2. `avail`/`unavail` intervals in seconds {"avail": 5, "unavail": 10}
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
    fn apply(&self) -> Option<ErrNo> {
        match self.cond {
            FlakeyCondition::Always{} => Some(self.errno),
            FlakeyCondition::Prob { prob } => {
                Some(self.errno).take_if(|_| rand::rng().random::<f32>() > prob)
            }
            FlakeyCondition::Interval { avail, unavail } => {
                let passed_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    - 1754140900000; // smaller value is float friendly                

                let rem = (passed_ms as f64 / 1000f64) % (avail + unavail);
                println!("passed {}, rem {}", passed_ms, rem);
                Some(self.errno).take_if(|_| rem > avail)
            }
        }
    }

    fn serialize(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}
