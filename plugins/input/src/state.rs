use logisim_plugin_api::{
    Properties, Signal,
    interaction::{InputEvent, Key},
};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct State {
    pub clock: u8,
    pub held: u32,
    pub queue: VecDeque<u8>,
    pub overflow: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            clock: 2,
            held: 0,
            queue: VecDeque::new(),
            overflow: false,
        }
    }
}
pub(crate) fn bit(v: Signal) -> u8 {
    if v.error != 0 {
        3
    } else if v.unknown != 0 {
        2
    } else {
        v.value as u8
    }
}
pub(crate) fn key_mask(key: Key) -> u32 {
    1 << Key::ALL.iter().position(|k| *k == key).unwrap()
}
impl State {
    pub fn decode(bytes: &[u8], capacity: usize) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        if bytes.len() < 11
            || &bytes[..4] != b"KEY\x01"
            || bytes[4] > 3
            || bytes[9] > 1
            || bytes[10] as usize > capacity
            || bytes.len() != 11 + bytes[10] as usize
            || bytes[11..].iter().any(|v| *v > 15)
        {
            return Err("Invalid keyboard state".into());
        }
        Ok(Self {
            clock: bytes[4],
            held: u32::from_le_bytes(bytes[5..9].try_into().unwrap()),
            overflow: bytes[9] != 0,
            queue: bytes[11..].iter().copied().collect(),
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = b"KEY\x01".to_vec();
        bytes.push(self.clock);
        bytes.extend(self.held.to_le_bytes());
        bytes.extend([u8::from(self.overflow), self.queue.len() as u8]);
        bytes.extend(&self.queue);
        bytes
    }
    pub fn release(&mut self) {
        self.held = 0;
        self.queue.clear();
        self.overflow = false;
    }
    pub fn sample(&mut self, p: &Properties, inputs: &[Signal]) {
        let clock = bit(inputs[0]);
        let edge = if p["trigger"] == "rising" {
            (0, 1)
        } else {
            (1, 0)
        };
        if bit(inputs[2]) == 1 {
            self.release();
        } else if (self.clock, clock) == edge && bit(inputs[1]) == 1 {
            self.queue.pop_front();
        }
        self.clock = clock;
    }
    pub fn input(&mut self, p: &Properties, capacity: usize, event: InputEvent) {
        let InputEvent::Key { key, pressed } = event else {
            self.release();
            return;
        };
        let mask = key_mask(key);
        if (self.held & mask != 0) == pressed {
            return;
        }
        if pressed {
            self.held |= mask;
        } else {
            self.held &= !mask;
        }
        for channel in 0..8 {
            if p[&format!("key{channel}")] != key.name() {
                continue;
            }
            if self.queue.len() == capacity {
                self.overflow = true;
            } else {
                self.queue.push_back(channel | (u8::from(pressed) << 3));
            }
        }
    }
    pub fn mask(&self, p: &Properties) -> u32 {
        (0..8).fold(0, |out, channel| {
            let name = &p[&format!("key{channel}")];
            let key = Key::ALL.iter().find(|k| k.name() == name).unwrap();
            out | (u32::from(self.held & key_mask(*key) != 0) << channel)
        })
    }
}
