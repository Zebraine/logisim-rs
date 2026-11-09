//! Momentary, instance-local two-axis input.
use crate::Simulator;
use logisim_core::{Id, Point};

// Measured outputs at each integer displacement -14..=14. Keep the observed
// quantization, including the skipped codes around the five-bit midpoint.
const AXES: [[u8; 29]; 4] = [
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    ],
    [
        1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7,
    ],
    [
        1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14,
        14, 15,
    ],
    [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
        28, 29, 30, 31,
    ],
];
pub(crate) fn axis_value(bits: u8, displacement: i32) -> u32 {
    u32::from(AXES[usize::from(bits.clamp(2, 5) - 2)][(displacement.clamp(-14, 14) + 14) as usize])
}

impl Simulator {
    pub fn joystick(&self, id: Id) -> Option<Point> {
        self.joysticks.get(&id).copied()
    }
    pub fn set_joystick(&mut self, id: Id, offset: Point) {
        let Some(current) = self.joysticks.get_mut(&id) else {
            return;
        };
        let offset = Point::new(offset.x.clamp(-14, 14), offset.y.clamp(-14, 14));
        if *current == offset {
            return;
        }
        *current = offset;
        self.limited = false;
        self.progress.restart();
        self.schedule_controls(None);
    }
    pub fn release_joysticks(&mut self) {
        let ids: Vec<_> = self.joysticks.keys().copied().collect();
        for id in ids {
            self.set_joystick(id, Point::ZERO);
        }
    }
}

#[cfg(test)]
mod tests;
