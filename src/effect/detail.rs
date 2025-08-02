use std::{collections::HashMap, time::Duration};
use tinyjson::JsonValue;

use crate::{effect::Effect, ftypes::ErrNo};

pub struct Delay {
    duration: Duration,
}

impl Delay {
    pub fn new(mut items: HashMap<String, JsonValue>) -> Result<Self, String> {
        if let JsonValue::Number(delay) = items.remove("delay").unwrap() {
            Ok(Delay {
                duration: Duration::from_millis(delay as u64),
            })
        } else {
            Err(String::default())
        }
    }
}

impl Effect for Delay {
    fn apply(&self) -> Option<ErrNo> {
        std::thread::sleep(self.duration);
        None
    }

    fn serialize(&self) -> Vec<(&'static str, JsonValue)> {
        vec![(
            "delay",
            JsonValue::Number(self.duration.as_millis() as f64),
        )]
    }
}
