//! Common port dispatch and world-coordinate transforms. Family modules own their layouts.
use crate::{
    Component, Extension, Facing, GRID, Gate, GateSize, Kind, Point, SplitAppearance, Ttl,
};

impl Component {
    pub fn input_count(&self) -> u8 {
        match self.kind {
            Kind::Gate(g) if g.unary() => 1,
            Kind::Gate(_) => self.inputs.clamp(2, 8),
            Kind::Parity(_) => self.inputs.clamp(2, 32),
            _ => 0,
        }
    }
    pub fn local_ports(&self) -> Vec<Point> {
        if self.kind.is_external() {
            return self.external_ports();
        }
        if matches!(self.kind, Kind::Ttl(_)) {
            return self
                .ttl_pins()
                .iter()
                .map(|p| Ttl::pin_position(*p))
                .collect();
        }
        if self.kind == Kind::PlaRom {
            return vec![Point::new(60, 0), Point::ZERO, Point::new(30, 30)];
        }
        if matches!(self.kind, Kind::Parity(_)) {
            return self.parity_ports();
        }
        if self.kind.is_transistor() {
            return self.transistor_ports();
        }
        if self.kind.is_controlled() {
            return self.controlled_ports();
        }
        if self.kind.is_io() {
            return self.io_ports();
        }
        if self.kind.is_memory() {
            return self.memory_terminals().iter().map(|p| p.offset).collect();
        }
        if matches!(self.kind, Kind::Plexer(_)) {
            return self
                .plexer_terminals()
                .into_iter()
                .map(|p| p.offset)
                .collect();
        }
        if let Some(ports) = self.arithmetic_ports() {
            return ports;
        }
        if let Some(ports) = self.sequential_ports() {
            return ports;
        }
        if matches!(self.kind, Kind::Subcircuit(_)) {
            return self.interface.ports.iter().map(|p| p.offset).collect();
        }
        if self.kind == Kind::Text {
            return vec![];
        }
        if self.kind == Kind::Splitter {
            let mut ports = self.splitter.ports();
            if matches!(self.facing, Facing::West | Facing::North) {
                ports[1..].reverse();
            }
            // Even centered layouts keep their extra branch on the world top/left side.
            // Rotating an asymmetric East layout alone shifts West/South by one grid cell.
            if matches!(
                self.splitter.appearance,
                SplitAppearance::Center | SplitAppearance::Legacy
            ) && self.splitter.fanout.is_multiple_of(2)
                && matches!(self.facing, Facing::West | Facing::South)
            {
                for port in &mut ports[1..] {
                    port.y += 10;
                }
            }
            return ports;
        }
        if self.kind == Kind::BitExtender {
            let mut ports = vec![Point::ZERO, Point::new(-40, 0)];
            if self.extension == Extension::Input {
                ports.push(Point::new(-20, -20));
            }
            return ports;
        }
        let count = self.input_count();
        let mut ports = vec![Point::ZERO];
        for i in 0..count {
            let y = if count == 1 {
                0
            } else {
                // Positions measured from the running reference for 2–8 inputs.
                let half = i32::from(count / 2);
                let index = i32::from(i) - half;
                let index = if count.is_multiple_of(2) && index >= 0 {
                    index + 1
                } else {
                    index
                };
                match (self.gate_size, count) {
                    (GateSize::Medium | GateSize::Wide, 2) | (GateSize::Medium, 3) => index * 20,
                    (GateSize::Wide, 3) => index * 30,
                    (GateSize::Wide, 4) => [-30, -10, 10, 30][usize::from(i)],
                    _ => index * GRID,
                }
            };
            let y = if matches!(self.facing, Facing::South | Facing::West) {
                -y
            } else {
                y
            };
            ports.push(Point::new(
                -self.gate_depth() - if self.gate_negated(i) { 10 } else { 0 },
                y,
            ));
        }
        ports
    }
    pub fn gate_width(&self) -> i32 {
        match self.kind {
            Kind::Gate(Gate::Buffer) | Kind::ControlledBuffer => 20,
            Kind::Gate(Gate::Not) | Kind::ControlledInverter => {
                if self.gate_size == GateSize::Wide {
                    30
                } else {
                    20
                }
            }
            Kind::Gate(_) | Kind::Parity(_) => match self.gate_size {
                GateSize::Narrow => 30,
                GateSize::Medium => 50,
                GateSize::Wide => 70,
            },
            _ => 0,
        }
    }
    pub fn gate_depth(&self) -> i32 {
        self.gate_width()
            + match self.kind {
                Kind::Gate(Gate::Nand | Gate::Nor | Gate::Xor) => 10,
                Kind::Gate(Gate::Xnor) => 20,
                _ => 0,
            }
    }
    pub fn ports(&self) -> Vec<Point> {
        self.local_ports()
            .into_iter()
            .map(|p| self.position + self.facing.transform(p))
            .collect()
    }
}
