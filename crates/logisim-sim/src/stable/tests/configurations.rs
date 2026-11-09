use crate::{Bit, Simulator, Value};
use logisim_core::{Arithmetic, BitSearch, Circuit, Component, Kind, Plexer, Point, Shift};

#[test]
fn arithmetic_and_plexer_option_edits_refresh_compiled_outputs_without_moving_ports() {
    for (kind, observed_port, before, after) in [
        (
            Kind::Arithmetic(Arithmetic::Comparator),
            3,
            Value::known(1, 0),
            Value::known(1, 1),
        ),
        (
            Kind::Arithmetic(Arithmetic::Shifter),
            0,
            Value::known(4, 2),
            Value::known(4, 4),
        ),
        (
            Kind::Arithmetic(Arithmetic::BitFinder),
            0,
            Value::known(2, 1),
            Value::known(2, 3),
        ),
        (
            Kind::Plexer(Plexer::Demultiplexer),
            3,
            Value::known(4, 0),
            Value::filled(4, Bit::Unknown),
        ),
        (
            Kind::Plexer(Plexer::PriorityEncoder),
            0,
            Value::filled(3, Bit::Unknown),
            Value::known(3, 0),
        ),
    ] {
        let mut circuit = Circuit::default();
        let mut component = Component::new(kind, Point::new(300, 300));
        component.bits = 4;
        component.plexing.enable = true;
        let mut inputs = vec![];
        for (port, at) in component.ports().into_iter().enumerate() {
            if component.drives_port(port) {
                continue;
            }
            let bits = component.port_width(port).unwrap();
            let mut pin = Component::new(Kind::Pin, at);
            pin.bits = bits;
            let value = match kind {
                Kind::Arithmetic(Arithmetic::Comparator) => {
                    if port == 1 {
                        8
                    } else {
                        7
                    }
                }
                Kind::Arithmetic(Arithmetic::Shifter) => {
                    if port == 1 {
                        9
                    } else {
                        1
                    }
                }
                Kind::Arithmetic(Arithmetic::BitFinder) => 10,
                Kind::Plexer(Plexer::Demultiplexer) => {
                    if port == 0 {
                        10
                    } else {
                        1
                    }
                }
                Kind::Plexer(Plexer::PriorityEncoder) => u32::from(port == 3),
                _ => unreachable!(),
            };
            inputs.push((circuit.add_component(pin), value));
        }
        let id = circuit.add_component(component);
        let mut event = Simulator::default();
        let mut compiled = Simulator::default();
        for sim in [&mut event, &mut compiled] {
            sim.synchronize(&circuit);
            for &(pin, value) in &inputs {
                sim.set_input(pin, value);
            }
        }
        event.advance(32);
        compiled.settle_compiled().unwrap();
        assert_eq!(
            compiled.port_value(id, observed_port),
            Some(before),
            "{kind:?}"
        );
        assert_eq!(event.port_value(id, observed_port), Some(before));

        let mut edited = circuit.component(id).unwrap().clone();
        let old_ports = edited.ports();
        match kind {
            Kind::Arithmetic(Arithmetic::Comparator) => edited.arithmetic.signed = false,
            Kind::Arithmetic(Arithmetic::Shifter) => edited.arithmetic.shift = Shift::LogicalRight,
            Kind::Arithmetic(Arithmetic::BitFinder) => {
                edited.arithmetic.search = BitSearch::HighestOne
            }
            Kind::Plexer(Plexer::Demultiplexer) => edited.plexing.three_state = true,
            Kind::Plexer(Plexer::PriorityEncoder) => edited.plexing.no_input_zero = true,
            _ => unreachable!(),
        }
        assert_eq!(edited.ports(), old_ports);
        circuit.replace_component(edited.clone());
        assert!(event.synchronize(&circuit));
        assert!(compiled.synchronize(&circuit));
        event.advance(32);
        compiled.settle_compiled().unwrap();
        assert_eq!(
            compiled.port_value(id, observed_port),
            Some(after),
            "{kind:?}"
        );
        assert_eq!(event.port_value(id, observed_port), Some(after));

        let logic = (edited.arithmetic_logic(), edited.plexer_logic());
        edited.position = edited.position + Point::new(0, 100);
        assert_eq!(logic, (edited.arithmetic_logic(), edited.plexer_logic()));
        circuit.replace_component(edited);
        // Geometry is still part of topology invalidation, even with identical logic.
        assert!(event.synchronize(&circuit));
        assert!(compiled.synchronize(&circuit));
        event.advance(32);
        compiled.settle_compiled().unwrap();
        for port in 0..circuit.component(id).unwrap().ports().len() {
            assert_eq!(
                compiled.port_value(id, port),
                event.port_value(id, port),
                "{kind:?} {port}"
            );
        }
    }
}
