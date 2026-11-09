use super::super::*;
use crate::{Bit, Value};
use logisim_core::{Component, Point, Pull};

#[test]
fn bus_drivers_resolve_each_bit_and_release_without_stale_values() {
    use Bit::{Error as E, One as H, Unknown as X, Zero as L};
    let states = [L, H, X, E];
    // Two strong drivers with a weak pull-up, including conflicts and release.
    let expected = [[L, E, L, E], [E, H, H, E], [L, H, H, E], [E, E, E, E]];
    for width in [1, 8, 32] {
        let mut circuit = Circuit::default();
        let mut pin = Component::new(Kind::Pin, Point::ZERO);
        pin.bits = width;
        let a = (circuit.add_component(pin.clone()), 0);
        let b = (circuit.add_component(pin), 0);
        let mut pull = Component::new(Kind::PullResistor, Point::ZERO);
        pull.bus.pull = Pull::One;
        circuit.add_component(pull);
        let network = Network::compile(&circuit);
        let net = network.ports[&a];
        let mut drives = BTreeMap::new();
        for i in 0..4 {
            for j in 0..4 {
                // Mixed states within a bus detect incorrect reuse of a bit.
                let left =
                    Value::from_bits((0..width).map(|bit| states[(i + usize::from(bit)) % 4]));
                let right =
                    Value::from_bits((0..width).map(|bit| states[(j + usize::from(bit)) % 4]));
                drives.insert(a, left);
                drives.insert(b, right);
                let want =
                    Value::from_bits((0..width).map(|bit| {
                        expected[(i + usize::from(bit)) % 4][(j + usize::from(bit)) % 4]
                    }));
                assert_eq!(network.resolve(&drives)[net], want);
                drives.remove(&a);
                let want =
                    Value::from_bits((0..width).map(|bit| expected[2][(j + usize::from(bit)) % 4]));
                assert_eq!(network.resolve(&drives)[net], want);
                drives.clear();
                assert_eq!(network.resolve(&drives)[net], Value::filled(width, H));
            }
        }
    }
}
