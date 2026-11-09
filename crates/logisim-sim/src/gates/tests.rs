use super::*;
use crate::Simulator;
use logisim_core::{Circuit, Facing, Point};
use logisim_core::{Component, Kind};

#[test]
fn negated_inputs_match_reference_four_state_and_disconnected_matrix() {
    let states = [
        Some(Bit::Zero),
        Some(Bit::One),
        Some(Bit::Unknown),
        Some(Bit::Error),
        None,
    ];
    for (gate, odd, rows) in [
        (
            Gate::And,
            false,
            ["01EE1", "00000", "0EEEE", "0EEEE", "01XEE"],
        ),
        (
            Gate::Nand,
            false,
            ["10EE0", "11111", "1EEEE", "1EEEE", "10EEE"],
        ),
        (
            Gate::Or,
            false,
            ["11111", "01EE0", "E1EEE", "E1EEE", "01XEE"],
        ),
        (
            Gate::Nor,
            false,
            ["00000", "10EE1", "E0EEE", "E0EEE", "10EEE"],
        ),
        (
            Gate::Xor,
            false,
            ["10EE1", "01EE0", "EEEEE", "EEEEE", "01EEE"],
        ),
        (
            Gate::Xor,
            true,
            ["10EE1", "01EE0", "EEEEE", "EEEEE", "01XEE"],
        ),
        (
            Gate::Xnor,
            false,
            ["01EE0", "10EE1", "EEEEE", "EEEEE", "10EEE"],
        ),
        (
            Gate::Xnor,
            true,
            ["01EE0", "10EE1", "EEEEE", "EEEEE", "10EEE"],
        ),
    ] {
        for facing in Facing::ALL {
            for bits in [1, 4, 32] {
                for (a, av) in states.iter().enumerate() {
                    for (b, bv) in states.iter().enumerate() {
                        let mut circuit = Circuit::default();
                        let mut gate_component =
                            Component::new(Kind::Gate(gate), Point::new(300, 300));
                        gate_component.facing = facing;
                        gate_component.bits = bits;
                        gate_component.gate_options.negated = 1;
                        gate_component.gate_options.xor_odd = odd;
                        let ports = gate_component.ports();
                        let output = circuit.add_component(gate_component);
                        let mut drives = vec![];
                        for (index, value) in [*av, *bv].into_iter().enumerate() {
                            if let Some(value) = value {
                                let mut pin = Component::new(Kind::Pin, ports[index + 1]);
                                pin.bits = bits;
                                drives
                                    .push((circuit.add_component(pin), Value::filled(bits, value)));
                            }
                        }
                        let mut sim = Simulator::default();
                        sim.synchronize(&circuit);
                        for (id, value) in drives {
                            sim.external_input(id, value);
                        }
                        sim.advance(32);
                        let expected = match rows[a].as_bytes()[b] {
                            b'0' => Bit::Zero,
                            b'1' => Bit::One,
                            b'X' => Bit::Unknown,
                            _ => Bit::Error,
                        };
                        assert_eq!(
                            sim.port_value(output, 0),
                            Some(Value::filled(bits, expected)),
                            "{gate:?} {odd} {a} {b}"
                        );
                        assert!(!sim.limited());
                    }
                }
            }
        }
    }
}

#[test]
fn xor_mode_changes_recompute_and_match_all_three_input_vectors() {
    for gate in [Gate::Xor, Gate::Xnor] {
        let mut circuit = Circuit::default();
        let mut component = Component::new(Kind::Gate(gate), Point::new(300, 300));
        component.gate_options.inputs = 3;
        component.bits = 8;
        let ports = component.ports();
        let id = circuit.add_component(component);
        // Eight simultaneous lanes enumerate all three-input combinations.
        for (port, value) in ports.into_iter().skip(1).zip([0xaa, 0xcc, 0xf0]) {
            let mut c = Component::new(Kind::Constant, port);
            c.bits = 8;
            c.constant = value;
            circuit.add_component(c);
        }
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        sim.advance(16);
        let invert = if gate == Gate::Xnor { 255 } else { 0 };
        assert_eq!(sim.port_value(id, 0), Some(Value::known(8, 0x16 ^ invert)));
        let mut changed = circuit.component(id).unwrap().clone();
        changed.gate_options.xor_odd = true;
        circuit.replace_component(changed);
        assert!(sim.synchronize(&circuit));
        sim.advance(16);
        assert_eq!(sim.port_value(id, 0), Some(Value::known(8, 0x96 ^ invert)));
    }
}
