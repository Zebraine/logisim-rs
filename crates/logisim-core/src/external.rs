//! Fixed interfaces for the two observed course-library components.
use crate::{Component, Kind, Point};

impl Kind {
    pub fn is_external(self) -> bool {
        matches!(self, Self::RegisterFile | Self::RiscVProbe)
    }
    pub fn external_identity(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::RegisterFile => Some(("edu.cornell.cs3410.Components", "RegisterFile")),
            Self::RiscVProbe => Some(("hust2020.Components", "RiscV Probe")),
            _ => None,
        }
    }
    pub fn default_external_library(self) -> &'static str {
        match self {
            Self::RegisterFile => "jar#cs3410.jar#edu.cornell.cs3410.Components",
            Self::RiscVProbe => "jar#riscv-probe.jar#hust2020.Components",
            _ => "",
        }
    }
}
impl Component {
    pub fn external_body(&self) -> [Point; 2] {
        if self.kind == Kind::RegisterFile {
            [Point::new(-160, -90), Point::new(0, 90)]
        } else {
            [Point::new(-65, -25), Point::new(65, 0)]
        }
    }
    /// A, B, W, CLK, WE, rW, rA, rB. Probe has one input.
    pub fn external_ports(&self) -> Vec<Point> {
        if self.kind == Kind::RegisterFile {
            [
                (0, -50),
                (0, 50),
                (-160, -10),
                (-160, 80),
                (-120, 90),
                (-90, 90),
                (-50, 90),
                (-30, 90),
            ]
            .map(|(x, y)| Point::new(x, y))
            .to_vec()
        } else {
            vec![Point::ZERO]
        }
    }
    pub fn external_port_label(&self, index: usize) -> &'static str {
        if self.kind == Kind::RegisterFile {
            [
                "Read data A",
                "Read data B",
                "Write data",
                "Clock",
                "Write enable",
                "Write address",
                "Read address A",
                "Read address B",
            ]
            .get(index)
            .copied()
            .unwrap_or("")
        } else {
            "Instruction"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_interfaces_and_trigger_edits_keep_wires() {
        let mut c = Component::new(Kind::RegisterFile, Point::new(300, 200));
        assert_eq!(
            c.ports(),
            [
                (300, 150),
                (300, 250),
                (140, 190),
                (140, 280),
                (180, 290),
                (210, 290),
                (250, 290),
                (270, 290)
            ]
            .map(|(x, y)| Point::new(x, y))
        );
        for (i, width) in [32, 32, 32, 1, 1, 5, 5, 5].into_iter().enumerate() {
            assert_eq!(c.port_width(i), Some(width));
            assert_eq!(c.drives_port(i), i < 2);
        }
        let mut circuit = crate::Circuit::default();
        c.id = circuit.add_component(c.clone());
        circuit.add_wire(c.ports()[0], c.ports()[0] + Point::new(50, 0));
        let wires = circuit.wires.clone();
        c.sequential.trigger = crate::Trigger::Low;
        circuit.replace_component(c);
        assert_eq!(circuit.wires, wires);
        let probe = Component::new(Kind::RiscVProbe, Point::ZERO);
        assert_eq!(probe.ports(), [Point::ZERO]);
        assert_eq!(probe.port_width(0), Some(32));
        assert!(!probe.drives_port(0));
    }
}
