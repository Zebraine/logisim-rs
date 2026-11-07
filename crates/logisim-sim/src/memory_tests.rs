use super::*;
use logisim_core::{Component, MemoryInterface, MemoryRole, Point, Project};
fn fixture(interface: MemoryInterface) -> (Circuit, Id, Vec<(MemoryRole, Id)>) {
    let mut c = Component::new(Kind::Ram, Point::new(200, 100));
    c.bits = 32;
    c.memory.address_bits = 24;
    c.memory.interface = interface;
    c.memory.simple = false;
    let mut circuit = Circuit::default();
    let inputs = c
        .memory_terminals()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 0 || interface != MemoryInterface::Separate)
        .map(|(i, p)| {
            let mut pin = Component::new(Kind::Pin, c.ports()[i]);
            pin.bits = p.bits;
            (p.role, circuit.add_component(pin))
        })
        .collect();
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}
fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
fn drive(s: &mut Simulator, pins: &[(MemoryRole, Id)], role: MemoryRole, value: Value) {
    if let Some((_, id)) = pins.iter().find(|(r, _)| *r == role) {
        s.external_input(*id, value);
        settle(s);
    }
}
#[test]
fn connected_memory_bus_writes_reads_resizes_and_resets() {
    use MemoryRole::*;
    for interface in MemoryInterface::ALL {
        let (mut circuit, id, pins) = fixture(interface);
        let mut s = Simulator::default();
        s.synchronize(&circuit);
        settle(&mut s);
        drive(&mut s, &pins, Address, Value::known(24, 0xff_ffff));
        drive(
            &mut s,
            &pins,
            if interface == MemoryInterface::Separate {
                Input
            } else {
                Data
            },
            Value::known(32, 0x8000_0001),
        );
        drive(&mut s, &pins, Store, Value::known(1, 1));
        drive(&mut s, &pins, Clock, Value::known(1, 1));
        assert_eq!(s.memory_contents(id).unwrap().get(0xff_ffff), 0x8000_0001);
        drive(&mut s, &pins, Load, Value::known(1, 1));
        // Release the shared data driver after returning the RAM to read mode.
        drive(&mut s, &pins, Data, Value::filled(32, Bit::Unknown));
        assert_eq!(s.port_value(id, 0).unwrap().number(), Some(0x8000_0001));
        s.set_memory_word(id, 0xff_ffff, 0xabcd_ef01);
        settle(&mut s);
        assert_eq!(s.port_value(id, 0).unwrap().number(), Some(0xabcd_ef01));
        drive(&mut s, &pins, Clock, Value::known(1, 0));
        for bits in [1, 32] {
            let mut c = circuit.component(id).unwrap().clone();
            c.bits = bits;
            circuit.replace_component(c);
            for (role, pin) in &pins {
                if matches!(role, Input | Data) {
                    circuit
                        .components
                        .iter_mut()
                        .find(|c| c.id == *pin)
                        .unwrap()
                        .bits = bits;
                }
            }
            s.synchronize(&circuit);
            settle(&mut s);
            assert_eq!(s.memory_contents(id).unwrap().get(0xff_ffff), 1);
        }
        s.set_memory_word(id, 0, 5);
        for width in [2, 24] {
            let mut c = circuit.component(id).unwrap().clone();
            c.memory.address_bits = width;
            circuit.replace_component(c);
            let address = pins.iter().find(|(r, _)| *r == Address).unwrap().1;
            circuit
                .components
                .iter_mut()
                .find(|c| c.id == address)
                .unwrap()
                .bits = width;
            s.synchronize(&circuit);
            settle(&mut s);
            assert_eq!(s.memory_contents(id).unwrap().get(0xff_ffff), 0);
            assert_eq!(s.memory_contents(id).unwrap().get(0), 5);
        }
        s.reset();
        settle(&mut s);
        // Synchronous RAM stays empty after reset. Asynchronous RAM may immediately write driven inputs.
        if interface != MemoryInterface::SharedAsync {
            assert_eq!(s.memory_contents(id).unwrap().boundaries().count(), 0);
        }
    }
}
#[test]
fn hierarchical_ram_contents_are_per_instance_while_rom_edits_are_document_data() {
    let mut project = Project::default();
    let root = project.main;
    let cell = project.add("cell".into()).unwrap();
    let circuit = &mut project.circuit_mut(cell).unwrap().circuit;
    let mut ram = Component::new(Kind::Ram, Point::new(200, 100));
    ram.memory.interface = MemoryInterface::Separate;
    let ram = circuit.add_component(ram);
    let mut rom = Component::new(Kind::Rom, Point::new(400, 100));
    rom.memory.contents.set(0, 0xa5);
    let rom = circuit.add_component(rom);
    let mut ids = vec![];
    for x in [100, 300] {
        let instance = project.instance(cell, Point::new(x, 300)).unwrap();
        ids.push(
            project
                .circuit_mut(root)
                .unwrap()
                .circuit
                .add_component(instance),
        );
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, root);
    sim.advance(128);
    sim.fill_memory(&[ids[0]], ram, 0, 256, 0x5a);
    sim.advance(128);
    assert_eq!(
        sim.view(&[ids[0]])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(255),
        0x5a
    );
    assert_eq!(
        sim.view(&[ids[1]])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(255),
        0
    );
    sim.synchronize(&project, root);
    sim.advance(128);
    assert_eq!(
        sim.view(&[ids[0]])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(0),
        0x5a
    );
    sim.reset();
    sim.advance(128);
    assert_eq!(
        sim.view(&[ids[0]])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(0),
        0
    );
    assert_eq!(
        project
            .circuit(cell)
            .unwrap()
            .circuit
            .component(rom)
            .unwrap()
            .memory
            .contents
            .get(0),
        0xa5
    );
    assert_eq!(
        project
            .circuit(cell)
            .unwrap()
            .circuit
            .component(ram)
            .unwrap()
            .memory
            .contents
            .boundaries()
            .count(),
        0
    );
}
