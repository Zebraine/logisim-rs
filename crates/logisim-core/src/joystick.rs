//! Joystick geometry uses world axes independently of its facing.
use crate::{Component, Facing, Point};

impl Component {
    pub fn joystick_center(&self) -> Point {
        self.position
            + match self.facing {
                Facing::East => Point::new(-15, 5),
                Facing::West => Point::new(15, 5),
                Facing::South => Point::new(-5, 15),
                Facing::North => Point::new(-5, -15),
            }
    }
}

#[cfg(test)]
mod tests;
