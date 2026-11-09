//! Execute independently built Wasm through both simulation protocols.
use logisim_core::{Bit, Value};
use logisim_plugin::{Plugin, Registry};
use logisim_plugin_api::{Evaluation, Event};
use logisim_ttl_combinational_plugin::{model::Chip, validation::*};
#[path = "check/networks.rs"]
mod networks;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("Expected module.wasm")?;
    let plugin = Plugin::load(&std::fs::read(path)?)?;
    assert_eq!(plugin.manifest().id, "org.logisim-rs.ttl-combinational");
    assert_eq!(plugin.manifest().tools.len(), 8);
    assert_eq!(
        plugin
            .library()
            .presentation
            .text("zh-CN", "Show internal structure"),
        "显示内部结构"
    );
    let mut count = 0;
    for mut c in plugin.manifest().components() {
        let chip = Chip::parse(&c.tool.id).unwrap();
        for internal in [false, true] {
            c.properties.insert("internal".into(), internal.to_string());
            let mut event = plugin.instantiate(&c)?;
            let mut stable = plugin
                .instantiate_stable(&c)?
                .ok_or("Missing combinational contract")?;
            assert!(stable.uses_linear_memory());
            let zero = signals(chip, &"0".repeat(chip.kind.inputs().len()));
            stable.reset(&zero)?;
            for (cases, expected) in [
                (binary(chip.kind), expected(chip.kind).0),
                (four(chip.kind), expected(chip.kind).1),
            ] {
                let mut hash = HASH_START;
                for values in cases {
                    let inputs = signals(chip, &values);
                    let slow = event.evaluate(
                        &c,
                        Evaluation {
                            tool: c.tool.id.clone(),
                            properties: c.properties.clone(),
                            inputs: inputs.clone(),
                            state: vec![],
                            event: Event::Propagate,
                        },
                    )?;
                    let fast = stable.outputs(&inputs, &[])?;
                    assert_eq!(slow.outputs, fast.outputs, "{} {values}", c.tool.id);
                    assert_eq!(slow.graphics, fast.graphics);
                    assert_eq!(!fast.graphics.is_empty(), internal);
                    fingerprint(&mut hash, chip, &fast.outputs);
                    count += 1;
                }
                assert_eq!(hash, expected, "{}", c.tool.id);
            }
            if chip.power {
                for a in "01XE".chars() {
                    for b in "01XE".chars() {
                        let mut inputs = zero.clone();
                        inputs[chip.index(chip.kind.count())] = Value::filled(1, decode(a)).into();
                        inputs[chip.index(chip.kind.count() / 2)] =
                            Value::filled(1, decode(b)).into();
                        let out = stable.outputs(&inputs, &[])?;
                        if (a, b) != ('1', '0') {
                            for pin in chip.kind.outputs() {
                                assert_eq!(
                                    Value::from(out.outputs[chip.index(*pin)]).bit(0),
                                    Bit::Unknown
                                );
                            }
                        }
                    }
                }
            }
            let t = stable.transition(&zero, &[])?;
            assert!(t.fixed_point && !t.outputs_changed && t.state.is_empty());
        }
    }
    let mut registry = Registry::default();
    registry.insert(plugin)?;
    networks::run(&registry);
    println!(
        "TTL combinational: {count} reference cases through event and linear-memory APIs; geometry, supply release, bus conflicts, hierarchy, roundtrip and missing-module checks passed"
    );
    Ok(())
}
