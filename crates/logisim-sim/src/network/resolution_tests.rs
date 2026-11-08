use super::*;
use logisim_core::{Circuit, Component, Kind, Point, Pull};

fn pin(c: &mut Circuit, at: Point, bits: u8) -> (Port, u8) {
    let mut pin = Component::new(Kind::Pin, at);
    pin.bits = bits;
    ((c.add_component(pin), 0), bits)
}

#[test]
fn changed_resolution_matches_full_scan_through_splitter_aliases_and_width_errors() {
    let mut seed = 7_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as u32
    };
    for width in [4, 8, 32] {
        for mismatch in [false, true] {
            let mut c = Circuit::default();
            let mut a = Component::new(Kind::Splitter, Point::ZERO);
            a.bits = width;
            a.splitter.fanout = 2;
            a.splitter.distribute(width, false);
            let mut b = Component::new(Kind::Splitter, a.ports()[2]);
            b.bits = width / 2;
            b.splitter.fanout = 2;
            b.splitter.distribute(b.bits, true);
            let mut sources = vec![];
            for splitter in [&a, &b] {
                for (i, at) in splitter.ports().into_iter().enumerate() {
                    let bits = splitter.port_width(i).unwrap();
                    sources.push(pin(&mut c, at, bits));
                    let mut pull = Component::new(Kind::PullResistor, at);
                    pull.bus.pull = [Pull::Zero, Pull::One, Pull::Error][i % 3];
                    c.add_component(pull);
                }
            }
            if mismatch {
                sources.push(pin(&mut c, b.ports()[2], 3));
            }
            for at in [a.ports()[0], Point::new(500, 500)] {
                let mut tunnel = Component::new(Kind::Tunnel, at);
                tunnel.bits = width;
                tunnel.label = "bus".into();
                c.add_component(tunnel);
            }
            sources.push(pin(&mut c, Point::new(500, 500), width));
            c.add_component(a);
            c.add_component(b);
            let network = Network::compile(&c);
            let mut drives = BTreeMap::new();
            let mut state = Resolution::new(&network, &drives);
            let mut values = state.values(&network);
            let mut changed = WorkSet::new(network.nets.len());
            assert_eq!(values, network.resolve(&drives));
            for _ in 0..256 {
                let before = values.clone();
                let mut batch = vec![];
                for &(port, width) in &sources {
                    match next() % 5 {
                        0 => continue,
                        1 => {
                            drives.remove(&port);
                            batch.push((port, None));
                        }
                        _ => {
                            let value = Value::from_bits((0..width).map(|_| {
                                [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error]
                                    [(next() % 4) as usize]
                            }));
                            drives.insert(port, value);
                            batch.push((port, Some(value)));
                        }
                    }
                }
                state.update(&network, batch, &mut values, &mut changed);
                assert_eq!(values, network.resolve(&drives));
                let expected: Vec<_> = values
                    .iter()
                    .zip(before)
                    .enumerate()
                    .filter_map(|(i, (a, b))| (*a != b).then_some(i))
                    .collect();
                let mut actual = changed.items.clone();
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
            // Reset discards cached strong drives but still applies all weak pulls.
            let reset = Resolution::new(&network, &BTreeMap::new());
            assert_eq!(reset.values(&network), network.resolve(&BTreeMap::new()));
        }
    }
}

#[test]
fn sparse_updates_skip_unrelated_nets_and_masked_driver_changes_stop_at_the_bus() {
    let mut c = Circuit::default();
    let sources: Vec<_> = (0..2048)
        .map(|i| pin(&mut c, Point::new(i * 10, 0), 32).0)
        .collect();
    let duplicate = pin(&mut c, Point::ZERO, 32).0;
    let network = Network::compile(&c);
    let mut drives: BTreeMap<_, _> = sources.iter().map(|p| (*p, Value::known(32, 0))).collect();
    drives.insert(duplicate, Value::known(32, 0));
    let mut state = Resolution::new(&network, &drives);
    let mut values = state.values(&network);
    let mut changed = WorkSet::new(network.nets.len());
    state.update(
        &network,
        [(sources[100], Some(Value::known(32, 1)))],
        &mut values,
        &mut changed,
    );
    assert_eq!(state.dirty_bits.items.len(), 32);
    assert_eq!(state.dirty_nets.items.len(), 1);
    assert_eq!(changed.items, [network.ports[&sources[100]]]);
    // Removing either of two identical drivers does not change the resolved bus.
    state.update(&network, [(duplicate, None)], &mut values, &mut changed);
    assert_eq!(state.dirty_bits.items.len(), 32);
    assert!(state.dirty_nets.items.is_empty());
    assert!(changed.items.is_empty());
    // Swapping the remaining driver in a single batch must not expose a transient X.
    state.update(
        &network,
        [(sources[0], None), (duplicate, Some(Value::known(32, 0)))],
        &mut values,
        &mut changed,
    );
    assert!(changed.items.is_empty());
    state.update(
        &network,
        [(duplicate, Some(Value::known(32, 0)))],
        &mut values,
        &mut changed,
    );
    assert!(state.dirty_bits.items.is_empty());
}
