use super::*;
use crate::ProjectSimulator;
use logisim_core::{Point, Project};
fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
#[test]
fn rgb_intensities_and_partial_unknowns_only_disable_the_affected_channel() {
    let mut c = Component::new(Kind::RgbLed, Point::ZERO);
    for multi in [false, true] {
        c.io.rgb_multibit = multi;
        let width = if multi { 8 } else { 1 };
        for v in [0, 1, 18, 64, 128, 255] {
            let p = [
                Value::known(width, v),
                Value::known(width, 1),
                Value::known(width, 0),
            ];
            assert_eq!(
                rgb(&c, &p),
                if multi {
                    [v as u8, 1, 0]
                } else {
                    [if v & 1 == 1 { 255 } else { 0 }, 255, 0]
                }
            );
        }
        for i in 0..3 {
            for bit in [Bit::Unknown, Bit::Error] {
                let mut p = [Value::known(width, 255); 3];
                p[i] = Value::from_bits(
                    (0..width).map(|j| if j == width / 2 { bit } else { Bit::One }),
                );
                let mut expected = [255; 3];
                expected[i] = 0;
                assert_eq!(rgb(&c, &p), expected);
            }
        }
    }
}
#[test]
fn matrix_modes_have_distinct_observed_bit_order_and_select_priority() {
    let mut c = Component::new(Kind::DotMatrix, Point::ZERO);
    c.io.matrix_columns = 3;
    c.io.matrix_rows = 4;
    let render = |cells: Vec<Bit>| {
        cells
            .into_iter()
            .map(|b| match b {
                Bit::One => '1',
                Bit::Zero => '0',
                _ => 'E',
            })
            .collect::<String>()
    };
    assert_eq!(
        render(matrix(
            &c,
            &[Value::known(4, 1), Value::known(4, 2), Value::known(4, 4)]
        )),
        "000001010100"
    );
    c.io.matrix_input = MatrixInput::Rows;
    assert_eq!(
        render(matrix(
            &c,
            &[
                Value::known(3, 1),
                Value::known(3, 2),
                Value::known(3, 4),
                Value::known(3, 7)
            ]
        )),
        "001010100111"
    );
    c.io.matrix_input = MatrixInput::Select;
    assert_eq!(
        render(matrix(&c, &[Value::known(3, 1), Value::known(4, 2)])),
        "000000100000"
    );
    for bit in [Bit::Unknown, Bit::Error] {
        assert_eq!(
            render(matrix(&c, &[Value::filled(3, bit), Value::known(4, 0)])),
            "000000000000"
        );
        assert_eq!(
            render(matrix(&c, &[Value::known(3, 0), Value::filled(4, bit)])),
            "EEEEEEEEEEEE"
        );
        assert_eq!(
            render(matrix(&c, &[Value::filled(3, bit), Value::known(4, 7)])),
            "000EEEEEEEEE"
        );
    }
    c.io.matrix_columns = 4;
    c.io.matrix_rows = 4;
    let partial = Value::from_bits([Bit::One, Bit::Zero, Bit::Unknown, Bit::One].into_iter());
    let select = Value::from_bits([Bit::One, Bit::Unknown, Bit::Zero, Bit::One].into_iter());
    assert_eq!(render(matrix(&c, &[partial, select])), "10E10000EEEE10E1");
    for mode in MatrixInput::ALL {
        c.io.matrix_input = mode;
        c.io.matrix_columns = 32;
        c.io.matrix_rows = 32;
        let ports = vec![Value::known(32, 0x80000001); c.ports().len()];
        let cells = matrix(&c, &ports);
        assert_eq!(cells.len(), 1024);
        assert_eq!(
            cells.iter().filter(|b| **b == Bit::One).count(),
            if mode == MatrixInput::Select { 4 } else { 64 }
        );
    }
}
fn fixture(persistence: u32) -> (Circuit, Id, Id) {
    let mut c = Circuit::default();
    let mut m = Component::new(Kind::DotMatrix, Point::new(200, 200));
    m.io.matrix_columns = 1;
    m.io.matrix_rows = 1;
    m.io.matrix_persistence = persistence;
    let display = c.add_component(m);
    let pin = c.add_component(Component::new(Kind::Pin, Point::new(200, 200)));
    (c, display, pin)
}
#[test]
fn persistence_uses_ticks_and_survives_errors_but_resets_and_resizes_safely() {
    for p in [0, 1, 3] {
        let (mut circuit, display, pin) = fixture(p);
        let mut s = Simulator::default();
        s.synchronize(&circuit);
        settle(&mut s);
        s.set_input(pin, 1);
        settle(&mut s);
        for _ in 0..5 {
            s.tick();
            settle(&mut s);
        }
        s.set_enabled(false);
        s.set_input(pin, 0);
        s.advance(128);
        assert_eq!(s.matrix_cells(display), Some(vec![Bit::One]));
        s.step();
        assert_eq!(
            s.matrix_cells(display),
            Some(vec![if p > 0 { Bit::One } else { Bit::Zero }])
        );
        s.set_enabled(true);
        for _ in 0..10 {
            settle(&mut s);
        }
        if p > 0 {
            s.external_input(pin, Value::filled(1, Bit::Unknown));
            settle(&mut s);
            assert_eq!(s.matrix_cells(display), Some(vec![Bit::Unknown]));
            s.set_input(pin, 0);
            settle(&mut s);
            assert_eq!(s.matrix_cells(display), Some(vec![Bit::One]));
            circuit.components[0].io.on_color = [1, 2, 3, 128];
            assert!(!s.synchronize(&circuit));
            for tick in 1..=p {
                assert!(s.tick());
                settle(&mut s);
                assert_eq!(
                    s.matrix_cells(display),
                    Some(vec![if tick < p { Bit::One } else { Bit::Zero }])
                );
            }
        }
        s.set_input(pin, 1);
        settle(&mut s);
        s.set_input(pin, 0);
        settle(&mut s);
        s.reset();
        settle(&mut s);
        assert_eq!(s.matrix_cells(display), Some(vec![Bit::Zero]));
        circuit.components[0].io.matrix_rows = 32;
        assert!(s.synchronize(&circuit));
        settle(&mut s);
        assert_eq!(s.matrix_cells(display).unwrap().len(), 32);
    }
}
#[test]
fn persistence_is_independent_in_nested_instances_and_cached_roots() {
    let mut project = Project::default();
    let root = project.main;
    let cell = project.add("cell".into()).unwrap();
    let (mut c, display, pin) = fixture(3);
    c.components.iter_mut().find(|c| c.id == pin).unwrap().kind = Kind::Button;
    project.circuit_mut(cell).unwrap().circuit = c;
    let mut ids = vec![];
    for x in [200, 400] {
        let instance = project.instance(cell, Point::new(x, 100)).unwrap();
        ids.push(
            project
                .circuit_mut(root)
                .unwrap()
                .circuit
                .add_component(instance),
        );
    }
    let mut s = ProjectSimulator::default();
    s.synchronize(&project, root);
    s.advance(128);
    s.set_button(&[ids[0]], pin, true);
    s.advance(128);
    s.set_button(&[ids[0]], pin, false);
    s.advance(128);
    assert_eq!(
        s.view(&[ids[0]]).unwrap().matrix_cells(display),
        Some(vec![Bit::One])
    );
    assert_eq!(
        s.view(&[ids[1]]).unwrap().matrix_cells(display),
        Some(vec![Bit::Zero])
    );
    s.tick();
    s.advance(128);
    s.synchronize(&project, cell);
    s.advance(128);
    assert_eq!(
        s.view(&[]).unwrap().matrix_cells(display),
        Some(vec![Bit::Zero])
    );
    s.synchronize(&project, root);
    s.advance(128);
    for _ in 0..2 {
        s.tick();
        s.advance(128);
    }
    assert_eq!(
        s.view(&[ids[0]]).unwrap().matrix_cells(display),
        Some(vec![Bit::Zero])
    );
}
