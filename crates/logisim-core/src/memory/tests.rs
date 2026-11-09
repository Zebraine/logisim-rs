use super::*;
use crate::Component;
#[test]
fn compressed_ranges_preserve_neighbors_snapshots_and_resize_without_expansion() {
    let mut data = MemoryContents::default();
    data.fill(0, 1 << 24, 0x8000_0001);
    assert_eq!(data.boundaries().count(), 2);
    let snapshot = data.clone();
    data.set(8, 7);
    data.fill(4, 8, 3);
    data.fill(5, 7, 0);
    assert_eq!(
        (0..10).map(|a| data.get(a)).collect::<Vec<_>>(),
        vec![
            0x8000_0001,
            0x8000_0001,
            0x8000_0001,
            0x8000_0001,
            3,
            0,
            0,
            3,
            7,
            0x8000_0001
        ]
    );
    assert_eq!(snapshot.get(5), 0x8000_0001);
    data.resize(2, 1);
    data.resize(24, 32);
    assert_eq!(
        (data.get(0), data.get(3), data.get(4), data.get(0xff_ffff)),
        (1, 1, 0, 0)
    );
    data.fill(0, 4, 0);
    assert_eq!(data.boundaries().count(), 0);
    data.set(0xff_ffff, u32::MAX);
    assert_eq!(data.get(1 << 24), 0);
    data.fill(0xff_ffff, 1 << 24, 0);
    assert_eq!(data, MemoryContents::default());
}
#[test]
fn observed_memory_ports_and_interface_edits_keep_wire_coordinates() {
    use crate::Circuit;
    for kind in [Kind::Rom, Kind::Ram] {
        for interface in MemoryInterface::ALL {
            for simple in [false, true] {
                let mut c = Component::new(kind, Point::new(200, 100));
                c.bits = 32;
                c.memory.address_bits = 24;
                c.memory.interface = interface;
                c.memory.simple = simple;
                let ports = c.memory_terminals();
                assert_eq!((ports[0].offset, ports[0].bits), (Point::ZERO, 32));
                assert_eq!((ports[1].offset, ports[1].bits), (Point::new(-140, 0), 24));
                assert_eq!(
                    c.memory_port(MemoryRole::Select).is_some(),
                    kind == Kind::Rom || !simple || interface != MemoryInterface::Separate
                );
                assert_eq!(
                    c.memory_port(MemoryRole::Clock).is_some(),
                    kind == Kind::Ram && interface != MemoryInterface::SharedAsync
                );
                assert_eq!(
                    c.memory_port(MemoryRole::Clear).is_some(),
                    kind == Kind::Ram && !simple
                );
                for (i, p) in ports.iter().enumerate() {
                    assert_eq!(c.port_width(i), Some(p.bits));
                }
                let mut circuit = Circuit::default();
                let id = circuit.add_component(c);
                circuit.add_wire(Point::new(110, 140), Point::new(110, 180));
                let wires = circuit.wires.clone();
                let mut changed = circuit.component(id).unwrap().clone();
                changed.memory.interface = MemoryInterface::Separate;
                changed.memory.simple = !simple;
                circuit.replace_component(changed);
                assert_eq!(circuit.wires, wires);
            }
        }
    }
}

#[test]
fn logical_memory_interfaces_keep_roles_widths_and_absent_ports_explicit() {
    use MemoryInterface::*;
    use MemoryRole::*;
    for (kind, interface, simple, expected) in [
        (Kind::Rom, SharedSync, true, vec![Data, Address, Select]),
        (
            Kind::Ram,
            SharedSync,
            true,
            vec![Data, Address, Select, Clock, Load],
        ),
        (
            Kind::Ram,
            SharedSync,
            false,
            vec![Data, Address, Select, Clock, Load, Clear],
        ),
        (
            Kind::Ram,
            SharedAsync,
            true,
            vec![Data, Address, Select, Load],
        ),
        (
            Kind::Ram,
            SharedAsync,
            false,
            vec![Data, Address, Select, Load, Clear],
        ),
        (Kind::Ram, Separate, true, vec![Data, Address, Input, Clock]),
        (
            Kind::Ram,
            Separate,
            false,
            vec![Data, Address, Input, Select, Clock, Store, Clear],
        ),
    ] {
        let mut c = Component::new(kind, Point::ZERO);
        c.bits = 32;
        c.memory.address_bits = 24;
        c.memory.interface = interface;
        c.memory.simple = simple;
        let logic = c.memory_logic().unwrap();
        assert_eq!(logic.capacity(), 1 << 24);
        assert_eq!(logic.ports().map(|p| p.role).collect::<Vec<_>>(), expected);
        for (index, &role) in expected.iter().enumerate() {
            let bits = match role {
                Data | Input => 32,
                Address => 24,
                _ => 1,
            };
            assert_eq!(logic.port(index), Some(MemoryPort { role, bits }));
            assert_eq!(logic.port_index(role), Some(index));
        }
        for role in [Data, Address, Input, Select, Clock, Load, Store, Clear] {
            if !expected.contains(&role) {
                assert_eq!(logic.port_index(role), None);
            }
        }
        assert_eq!(logic.port(expected.len()), None);
        assert_eq!(logic.port(usize::MAX), None);
        c.position = Point::new(700, 300);
        c.label = "storage".into();
        c.memory.contents.set(0, 7);
        assert_eq!(c.memory_logic(), Some(logic));
        if kind == Kind::Rom {
            c.memory.interface = Separate;
            c.memory.simple = false;
            assert_eq!(c.memory_logic(), Some(logic));
        }
        c.bits = 0;
        c.memory.address_bits = 0;
        let logic = c.memory_logic().unwrap();
        assert_eq!(logic.capacity(), 4);
        assert_eq!(logic.port(0).unwrap().bits, 1);
        assert_eq!(logic.port(1).unwrap().bits, 2);
    }
    let pin = Component::new(Kind::Pin, Point::ZERO);
    assert_eq!(pin.memory_logic(), None);
    assert_eq!(pin.memory_port(Data), None);
}
