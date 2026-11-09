use super::super::*;
use crate::ProjectSimulator;
use logisim_core::{Circuit, Point, Project};
fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
#[test]
fn switch_transmits_four_states_only_forward_with_pause_reset_and_width_edits() {
    for width in [1, 4, 32] {
        for bit in [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error] {
            for reverse in [false, true] {
                let mut circuit = Circuit::default();
                let mut c = Component::new(Kind::Switch, Point::new(200, 100));
                c.bits = width;
                let id = circuit.add_component(c);
                let mut pin =
                    Component::new(Kind::Pin, Point::new(if reverse { 200 } else { 180 }, 100));
                pin.bits = width;
                let source_id = circuit.add_component(pin);
                let mut sim = Simulator::default();
                sim.synchronize(&circuit);
                settle(&mut sim);
                let source = Value::from_bits((0..width).map(|i| {
                    if i == width - 1 {
                        bit
                    } else if i % 2 == 0 {
                        Bit::One
                    } else {
                        Bit::Zero
                    }
                }));
                let driven = if reverse { 0 } else { 1 };
                sim.external_input(source_id, source);
                settle(&mut sim);
                if !reverse {
                    assert_eq!(
                        sim.port_value(id, 0),
                        Some(Value::filled(width, Bit::Unknown))
                    );
                }
                sim.set_enabled(false);
                sim.toggle_input(id, 0);
                assert_eq!(sim.input(id).unwrap().number(), Some(1));
                sim.set_enabled(true);
                settle(&mut sim);
                assert_eq!(
                    sim.port_value(id, 1 - driven),
                    Some(if reverse {
                        Value::filled(width, Bit::Unknown)
                    } else {
                        source
                    })
                );
                circuit.components[0].bits = if width == 32 { 4 } else { 32 };
                sim.synchronize(&circuit);
                settle(&mut sim);
                assert_eq!(sim.input(id), Some(Value::known(1, 1)));
                sim.reset();
                settle(&mut sim);
                assert_eq!(sim.input(id), Some(Value::known(1, 0)));
                assert_eq!(
                    sim.port_value(id, 0),
                    Some(Value::filled(circuit.components[0].bits, Bit::Unknown))
                );
            }
        }
    }
}
#[test]
fn dip_outputs_are_separate_and_resize_truncates_then_zero_extends() {
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::DipSwitch, Point::ZERO);
    c.io.switch_count = 32;
    let id = circuit.add_component(c);
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    settle(&mut sim);
    sim.toggle_input(id, 0);
    sim.toggle_input(id, 31);
    settle(&mut sim);
    for i in 0..32 {
        assert_eq!(
            sim.port_value(id, i),
            Some(Value::known(1, u32::from(i == 0 || i == 31)))
        );
    }
    circuit.components[0].io.on_color = [1, 2, 3, 128];
    assert!(!sim.synchronize(&circuit));
    circuit.components[0].io.switch_count = 1;
    sim.synchronize(&circuit);
    settle(&mut sim);
    assert_eq!(sim.input(id), Some(Value::known(1, 1)));
    circuit.components[0].io.switch_count = 32;
    sim.synchronize(&circuit);
    settle(&mut sim);
    assert_eq!(sim.input(id), Some(Value::known(32, 1)));
    assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 0)));
    sim.set_enabled(false);
    sim.toggle_input(id, 31);
    sim.advance(64);
    assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 0)));
    sim.step();
    assert_eq!(sim.port_value(id, 31), Some(Value::known(1, 1)));
    sim.reset();
    sim.set_enabled(true);
    settle(&mut sim);
    assert_eq!(sim.input(id), Some(Value::known(32, 0)));
}
#[test]
fn slider_is_document_state_shared_across_instances_switches_are_independent() {
    let mut p = Project::default();
    let root = p.main;
    let cell = p.add("Cell".into()).unwrap();
    let c = &mut p.circuit_mut(cell).unwrap().circuit;
    let sw = c.add_component(Component::new(Kind::Switch, Point::new(200, 100)));
    let dip = c.add_component(Component::new(Kind::DipSwitch, Point::new(200, 200)));
    let mut slider = Component::new(Kind::Slider, Point::new(200, 400));
    slider.bits = 32;
    slider.io.slider_value = 0x80000000;
    let sl = c.add_component(slider);
    let mut ids = vec![];
    for x in [100, 300] {
        let instance = p.instance(cell, Point::new(x, 100)).unwrap();
        ids.push(p.circuit_mut(root).unwrap().circuit.add_component(instance));
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.advance(128);
    sim.toggle_input_at(&[ids[0]], sw, 0);
    sim.toggle_input_at(&[ids[1]], dip, 3);
    sim.advance(128);
    for (i, id) in ids.iter().enumerate() {
        let s = sim.view(&[*id]).unwrap();
        assert_eq!(s.input(sw).unwrap().number(), Some(u32::from(i == 0)));
        assert_eq!(
            s.input(dip).unwrap().number(),
            Some(if i == 1 { 8 } else { 0 })
        );
        assert_eq!(s.port_value(sl, 0).unwrap().number(), Some(0x80000000));
        assert!(s.input(sl).is_none());
    }
    p.circuit_mut(cell)
        .unwrap()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.id == sl)
        .unwrap()
        .io
        .slider_value = u32::MAX;
    sim.synchronize(&p, root);
    sim.advance(128);
    for id in &ids {
        assert_eq!(
            sim.view(&[*id])
                .unwrap()
                .port_value(sl, 0)
                .unwrap()
                .number(),
            Some(u32::MAX)
        );
    }
    assert_eq!(
        sim.view(&[ids[0]]).unwrap().input(sw).unwrap().number(),
        Some(1)
    );
    sim.reset();
    sim.advance(128);
    for id in ids {
        let s = sim.view(&[id]).unwrap();
        assert_eq!(s.input(sw).unwrap().number(), Some(0));
        assert_eq!(s.input(dip).unwrap().number(), Some(0));
        assert_eq!(s.port_value(sl, 0).unwrap().number(), Some(u32::MAX));
    }
}
