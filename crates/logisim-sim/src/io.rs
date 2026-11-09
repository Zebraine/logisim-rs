use crate::{Bit, Simulator, Value};
use logisim_core::{Component, Id, Kind};
pub(crate) fn control_bits(c: &Component) -> Option<u8> {
    match c.kind {
        Kind::Pin if !c.output => Some(c.bits.clamp(1, 32)),
        Kind::Button | Kind::Switch => Some(1),
        Kind::DipSwitch => Some(c.io.switch_count.clamp(1, 32)),
        _ => None,
    }
}
impl Simulator {
    pub(crate) fn schedule_controls(&mut self, changed: Option<&crate::work::WorkSet>) {
        let outputs: Vec<_> = self
            .plan
            .controls
            .iter()
            .filter(|entry| entry.affected(changed))
            .flat_map(|entry| {
                let c = &self.circuit.components[entry.component];
                let state = self.inputs.get(&c.id).and_then(|v| v.number()).unwrap_or(0);
                match c.kind {
                    Kind::Switch => vec![(
                        (c.id, 0),
                        if state != 0 {
                            entry.port(1, &self.values)
                        } else {
                            Value::filled(c.bits, Bit::Unknown)
                        },
                    )],
                    Kind::DipSwitch => (0..c.io.switch_count.clamp(1, 32))
                        .map(|i| ((c.id, usize::from(i)), Value::known(1, (state >> i) & 1)))
                        .collect(),
                    Kind::Joystick => {
                        let offset = self.joystick(c.id).unwrap_or_default();
                        [offset.x, offset.y]
                            .into_iter()
                            .enumerate()
                            .map(|(i, d)| {
                                (
                                    (c.id, i),
                                    Value::known(c.bits, crate::joystick::axis_value(c.bits, d)),
                                )
                            })
                            .collect()
                    }
                    Kind::Slider => vec![(
                        (c.id, 0),
                        Value::known(c.bits, c.io.slider_value.min(c.slider_max())),
                    )],
                    _ => vec![],
                }
            })
            .collect();
        for (port, value) in outputs {
            self.drive_port(port, value);
        }
    }
    pub fn set_button(&mut self, id: Id, pressed: bool) {
        if self
            .circuit
            .component(id)
            .is_some_and(|c| c.kind == Kind::Button)
        {
            self.set_input(id, u32::from(pressed));
        }
    }
    pub fn release_buttons(&mut self) {
        let ids: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Button)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.set_button(id, false);
        }
    }
    /// Segment order: middle, upper-left, top, upper-right, lower-left, bottom, lower-right, dot.
    pub fn indicator(&self, id: Id) -> Option<[bool; 8]> {
        let c = self.circuit.component(id)?;
        if !matches!(c.kind, Kind::Led | Kind::SevenSegment | Kind::HexDisplay) {
            return None;
        }
        let ports = (0..c.ports().len())
            .map(|i| self.port_value(id, i))
            .collect::<Option<Vec<_>>>()?;
        Some(evaluate(c, &ports))
    }
}
fn evaluate(c: &Component, ports: &[crate::Value]) -> [bool; 8] {
    let mut lights = [false; 8];
    match c.kind {
        Kind::Led => {
            lights[0] = ports[0].bit(0)
                == if c.io.active_high {
                    Bit::One
                } else {
                    Bit::Zero
                }
        }
        Kind::SevenSegment => {
            for (light, value) in lights.iter_mut().zip(ports) {
                *light = (value.bit(0) == Bit::One) == c.io.active_high;
            }
        }
        Kind::HexDisplay => {
            let mask = ports[0].number().filter(|v| *v < 16).map_or(1, |v| {
                [
                    0x7e, 0x48, 0x3d, 0x6d, 0x4b, 0x67, 0x77, 0x4c, 0x7f, 0x4f, 0x5f, 0x73, 0x36,
                    0x79, 0x37, 0x17,
                ][v as usize]
            });
            for (i, light) in lights.iter_mut().enumerate().take(7) {
                *light = mask & (1 << i) != 0;
            }
            lights[7] = ports[1].bit(0) == Bit::One;
        }
        _ => {}
    }
    lights
}

#[cfg(test)]
mod tests;
