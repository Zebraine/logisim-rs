//! Deterministic document construction; no simulator callbacks or custom component kinds.
mod expr;
mod routing;
pub use expr::{E, Op, any};
use logisim_core::*;
use routing::{fanout, port_side};
use std::collections::{BTreeMap, HashMap};

const COLUMNS: usize = 14;

pub struct Builder {
    pub circuit: Circuit,
    cache: HashMap<E, String>,
    widths: BTreeMap<String, u8>,
    drivers: BTreeMap<String, usize>,
    cell: usize,
    sequence: usize,
}
impl Builder {
    pub fn new(title: &str) -> Self {
        let mut b = Self {
            circuit: Circuit::default(),
            cache: HashMap::new(),
            widths: BTreeMap::new(),
            drivers: BTreeMap::new(),
            cell: 0,
            sequence: 0,
        };
        b.text(title, Point::new(30, 10), 22);
        b
    }
    pub fn text(&mut self, text: &str, at: Point, size: u8) {
        let mut c = Component::new(Kind::Text, at);
        c.label = text.into();
        c.font.size = size;
        self.circuit.add_component(c);
    }
    pub fn section(&mut self, title: &str) {
        self.cell = self.cell.div_ceil(COLUMNS) * COLUMNS;
        let y = 160 + (self.cell / COLUMNS) as i32 * 280;
        self.text(title, Point::new(30, y - 45), 18);
        self.cell += COLUMNS;
    }
    pub fn add(&mut self, mut c: Component) -> Id {
        c.position = Point::new(
            240 + (self.cell % COLUMNS) as i32 * 480,
            160 + (self.cell / COLUMNS) as i32 * 280,
        );
        self.cell += 1;
        self.circuit.add_component(c)
    }
    fn net(&mut self, name: &str, bits: u8, driver: bool) {
        if let Some(width) = self.widths.insert(name.into(), bits) {
            assert_eq!(width, bits, "net {name}");
        }
        if driver {
            *self.drivers.entry(name.into()).or_default() += 1;
        }
    }
    pub fn attach(&mut self, id: Id, port: usize, name: &str, driver: bool) {
        let c = self.circuit.component(id).unwrap();
        let bits = c.port_width(port).unwrap();
        let facing = port_side(c, port);
        let route = fanout(c, port, facing);
        let end = *route.last().unwrap();
        self.tunnel(end, name, bits, facing, driver);
        for pair in route.windows(2) {
            self.circuit.add_wire(pair[0], pair[1]);
        }
    }
    pub fn tunnel(&mut self, at: Point, name: &str, bits: u8, facing: Facing, driver: bool) {
        self.net(name, bits, driver);
        let mut t = Component::new(Kind::Tunnel, at);
        t.label = name.into();
        t.font.size = 9;
        t.bits = bits;
        t.facing = facing;
        self.circuit.add_component(t);
    }
    pub fn input(&mut self, name: &str, bits: u8) -> E {
        let mut p = Component::new(Kind::Pin, Point::ZERO);
        p.bits = bits;
        p.label = name.into();
        let id = self.add(p);
        self.attach(id, 0, name, true);
        E::net(name, bits)
    }
    pub fn output(&mut self, name: &str, e: &E) {
        let net = self.compile(e);
        let mut p = Component::new(Kind::Pin, Point::ZERO);
        p.bits = e.bits;
        p.label = name.into();
        p.output = true;
        p.facing = Facing::West;
        let id = self.add(p);
        self.attach(id, 0, &net, false);
    }
    pub fn connect(&mut self, id: Id, port: usize, e: &E) {
        let net = self.compile(e);
        self.attach(id, port, &net, false);
    }
    fn emit(&mut self, mut c: Component, inputs: &[(usize, &E)], port: usize, bits: u8) -> String {
        let nets: Vec<_> = inputs.iter().map(|(p, e)| (*p, self.compile(e))).collect();
        let name = format!("n{}", self.sequence);
        self.sequence += 1;
        c.label = name.clone();
        c.font.size = 10;
        let id = self.add(c);
        for (port, net) in nets {
            self.attach(id, port, &net, false);
        }
        self.attach(id, port, &name, true);
        assert_eq!(self.widths[&name], bits);
        name
    }
    pub fn compile(&mut self, e: &E) -> String {
        if let Op::Net(name) = e.op.as_ref() {
            return name.clone();
        }
        if let Some(n) = self.cache.get(e) {
            return n.clone();
        }
        let mut c = Component::new(Kind::Gate(Gate::Buffer), Point::ZERO);
        c.bits = e.bits;
        let result = match e.op.as_ref() {
            Op::Net(_) => unreachable!(),
            Op::Constant(n) => {
                c.kind = Kind::Constant;
                c.constant = *n;
                self.emit(c, &[], 0, e.bits)
            }
            Op::Not(a) => {
                c.kind = Kind::Gate(Gate::Not);
                self.emit(c, &[(1, a)], 0, e.bits)
            }
            Op::And(a, b) | Op::Or(a, b) | Op::Xor(a, b) => {
                c.kind = Kind::Gate(match e.op.as_ref() {
                    Op::And(..) => Gate::And,
                    Op::Or(..) => Gate::Or,
                    _ => Gate::Xor,
                });
                c.gate_options.xor_odd = true;
                self.emit(c, &[(1, a), (2, b)], 0, e.bits)
            }
            Op::Add(a, b) | Op::Sub(a, b) => {
                c.kind = Kind::Arithmetic(if matches!(e.op.as_ref(), Op::Add(..)) {
                    Arithmetic::Adder
                } else {
                    Arithmetic::Subtractor
                });
                self.emit(c, &[(1, a), (2, b), (3, &E::num(1, 0))], 0, e.bits)
            }
            Op::Eq(a, b) => {
                c.kind = Kind::Arithmetic(Arithmetic::Comparator);
                c.bits = a.bits;
                c.arithmetic.signed = false;
                self.emit(c, &[(1, a), (2, b)], 0, 1)
            }
            Op::Mux(s, a, b) => {
                c.kind = Kind::Plexer(Plexer::Multiplexer);
                self.emit(c, &[(1, s), (2, b), (3, a)], 0, e.bits)
            }
            Op::Slice(a, start) => {
                c.kind = Kind::Splitter;
                c.bits = a.bits;
                c.splitter.fanout = 1;
                c.splitter.mapping.fill(None);
                for bit in *start..*start + e.bits {
                    c.splitter.mapping[bit as usize] = Some(0);
                }
                self.emit(c, &[(0, a)], 1, e.bits)
            }
            Op::Join(hi, lo) => {
                c.kind = Kind::Splitter;
                c.splitter.fanout = 2;
                c.splitter.mapping.fill(None);
                for bit in 0..e.bits {
                    c.splitter.mapping[bit as usize] = Some(u8::from(bit >= lo.bits));
                }
                self.emit(c, &[(1, lo), (2, hi)], 0, e.bits)
            }
        };
        self.cache.insert(e.clone(), result.clone());
        result
    }
    pub fn register(&mut self, name: &str, d: &E, en: &E, clear: &E) {
        let mut c = Component::new(Kind::Register, Point::ZERO);
        c.bits = d.bits;
        c.label = name.into();
        c.sequential.select_high = true;
        let id = self.add(c);
        self.attach(id, 0, name, true);
        for (p, e) in [
            (1, d.clone()),
            (2, E::net("CLK", 1)),
            (3, en.clone()),
            (4, E::num(1, 1)),
            (5, E::num(1, 0)),
            (6, clear.clone()),
        ] {
            self.connect(id, p, &e);
        }
    }
    pub fn memory(&mut self, name: &str, bits: u8, address: &E, data: &E, write: &E, clear: &E) {
        let mut c = Component::new(Kind::Ram, Point::ZERO);
        c.bits = bits;
        c.label = name.into();
        c.memory.interface = MemoryInterface::Separate;
        c.memory.simple = false;
        c.memory.select_high = true;
        let id = self.add(c);
        self.attach(id, 0, name, true);
        for (role, e) in [
            (MemoryRole::Address, address.clone()),
            (MemoryRole::Input, data.clone()),
            (MemoryRole::Select, E::num(1, 1)),
            (MemoryRole::Clock, E::net("CLK", 1)),
            (MemoryRole::Store, write.clone()),
            (MemoryRole::Clear, clear.clone()),
        ] {
            let port = self
                .circuit
                .component(id)
                .unwrap()
                .memory_port(role)
                .unwrap();
            self.connect(id, port, &e);
        }
    }
    pub fn finish(mut self) -> Circuit {
        for net in self.widths.keys() {
            assert_eq!(self.drivers.get(net), Some(&1), "driver count for {net}");
        }
        self.circuit.normalize_wires();
        self.circuit
    }
}
