//! Trees occupy disjoint vertical bands. Input rails are left of every gate;
//! crossings have no endpoints, while actual fanout taps always have endpoints.
use super::{
    Analysis, Error, MAX_GENERATED_ITEMS, Result,
    expression::{Node, Op},
};
use crate::{Circuit, Component, Facing, Gate, Kind, Point};

struct Layout {
    circuit: Circuit,
    taps: Vec<Vec<Point>>,
    leaf_x: i32,
    next_y: i32,
}
impl Layout {
    fn check(&self) -> Result<()> {
        if self.circuit.components.len() + self.circuit.wires.len() > MAX_GENERATED_ITEMS {
            Err(Error::new(
                "Generated circuit exceeds the 4096-item limit. Simplify the logic or split the outputs.",
            ))
        } else {
            Ok(())
        }
    }
    fn leaf(&mut self) -> Point {
        let point = Point::new(self.leaf_x, self.next_y);
        self.next_y += 80;
        point
    }
    fn route(&mut self, from: Point, to: Point, elbow: i32) {
        self.circuit.add_wire(from, Point::new(elbow, from.y));
        self.circuit
            .add_wire(Point::new(elbow, from.y), Point::new(elbow, to.y));
        self.circuit.add_wire(Point::new(elbow, to.y), to);
    }
    fn node(&mut self, node: &Node) -> Result<Point> {
        self.check()?;
        let point = match node {
            Node::Input(i) => {
                let p = self.leaf();
                self.taps[*i].push(p);
                p
            }
            Node::Constant(value) => {
                let p = self.leaf();
                let mut c = Component::new(Kind::Constant, p);
                c.constant = u32::from(*value);
                self.circuit.add_component(c);
                p
            }
            Node::Not(a) => {
                let source = self.node(a)?;
                let p = source + Point::new(100, 0);
                let c = Component::new(Kind::Gate(Gate::Not), p);
                self.circuit.add_wire(source, c.ports()[1]);
                self.circuit.add_component(c);
                p
            }
            Node::Binary(op, a, b) => {
                let a = self.node(a)?;
                let b = self.node(b)?;
                let p = Point::new(a.x.max(b.x) + 140, ((a.y + b.y) / 20) * 10);
                let gate = match op {
                    Op::And => Gate::And,
                    Op::Or => Gate::Or,
                    Op::Xor => Gate::Xor,
                };
                let c = Component::new(Kind::Gate(gate), p);
                let ports = c.ports();
                self.route(a, ports[1], p.x - 70);
                self.route(b, ports[2], p.x - 70);
                self.circuit.add_component(c);
                p
            }
        };
        self.check()?;
        Ok(point)
    }
}
pub(super) fn generate(analysis: &Analysis) -> Result<Circuit> {
    let mut layout = Layout {
        circuit: Circuit::default(),
        taps: vec![vec![]; analysis.inputs.len()],
        leaf_x: 300 + analysis.inputs.len() as i32 * 30,
        next_y: 120 + analysis.inputs.len() as i32 * 60,
    };
    let mut roots = Vec::new();
    for i in 0..analysis.outputs.len() {
        roots.push(layout.node(&analysis.expression(i)?.0)?);
        layout.next_y += 80;
    }
    let output_x = roots.iter().map(|p| p.x).max().unwrap() + 100;
    for (output, root) in analysis.outputs.iter().zip(roots) {
        let mut pin = Component::new(Kind::Pin, Point::new(output_x, root.y));
        pin.output = true;
        pin.facing = Facing::West;
        pin.label = output.name.clone();
        layout.circuit.add_wire(root, pin.position);
        layout.circuit.add_component(pin);
    }
    for (i, name) in analysis.inputs.iter().enumerate() {
        let mut pin = Component::new(Kind::Pin, Point::new(80, 80 + i as i32 * 60));
        pin.label = name.clone();
        let start = pin.position;
        layout.circuit.add_component(pin);
        if let Some(last) = layout.taps[i].last() {
            let x = 200 + i as i32 * 30;
            layout.circuit.add_wire(start, Point::new(x, start.y));
            layout
                .circuit
                .add_wire(Point::new(x, start.y), Point::new(x, last.y));
            for tap in &layout.taps[i] {
                layout.circuit.add_wire(Point::new(x, tap.y), *tap);
            }
        }
    }
    layout.circuit.normalize_wires();
    layout.check()?;
    Ok(layout.circuit)
}
