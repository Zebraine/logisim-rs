//! Regression checks execute the actual Wasm artifact through event and linear-memory APIs.
use logisim_core::{Bit, Circuit, Component, Facing, Kind, Point, Value};
use logisim_plugin::{Plugin, Registry};
use logisim_plugin_api::{Evaluation, Event, Signal};
use logisim_ttl_plugin::model::{Chip, Ttl};
#[path = "check/networks.rs"]
mod networks;
fn decode(b: u8) -> Bit {
    match b {
        b'0' => Bit::Zero,
        b'1' => Bit::One,
        b'X' => Bit::Unknown,
        _ => Bit::Error,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: ttl-plugin-check module.wasm")?;
    let bytes = std::fs::read(path)?;
    let plugin = Plugin::load(&bytes)?;
    assert_eq!(plugin.manifest().id, "org.logisim-rs.ttl");
    assert_eq!(plugin.manifest().tools.len(), 14);
    assert_eq!(
        plugin
            .library()
            .presentation
            .text("zh-CN", "Show internal structure"),
        "显示内部结构"
    );
    for mut c in plugin.manifest().components() {
        let chip = Chip::parse(&c.tool.id).unwrap();
        let expected = match chip.kind {
            Ttl::Ic7400 => b"111110EE1EEE1EEE",
            Ttl::Ic7402 => b"10EE0000E0EEE0EE",
            Ttl::Ic7404 => b"11110000EEEEEEEE",
            Ttl::Ic7408 => b"000001EE0EEE0EEE",
            Ttl::Ic7432 => b"01EE1111E1EEE1EE",
            Ttl::Ic7486 => b"01EE10EEEEEEEEEE",
            Ttl::Ic747266 => b"10EE01EEEEEEEEEE",
        };
        for internal in [false, true] {
            c.properties.insert("internal".into(), internal.to_string());
            let mut event = plugin.instantiate(&c)?;
            let mut stable = plugin
                .instantiate_stable(&c)?
                .ok_or("Missing stable contract")?;
            assert!(stable.uses_linear_memory());
            let mut inputs = vec![Signal::from(Value::known(1, 0)); c.tool.ports.len()];
            if chip.power {
                inputs[chip.index(14)] = Value::known(1, 1).into();
            }
            stable.reset(&inputs)?;
            for step in 0..16 {
                for (g, &[a, b, _]) in chip.kind.gates().iter().enumerate() {
                    let i = (step + g) % 16;
                    inputs[chip.index(a)] = Value::filled(1, decode(b"01XE"[i / 4])).into();
                    if a != b {
                        inputs[chip.index(b)] = Value::filled(1, decode(b"01XE"[i % 4])).into();
                    }
                }
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
                assert_eq!(slow.outputs, fast.outputs);
                assert_eq!(slow.graphics, fast.graphics);
                assert!(slow.state.is_empty());
                assert_eq!(!fast.graphics.is_empty(), internal);
                for (g, &[_, _, out]) in chip.kind.gates().iter().enumerate() {
                    assert_eq!(
                        Value::from(fast.outputs[chip.index(out)]).bit(0),
                        decode(expected[(step + g) % 16]),
                        "{} step {step} gate {g}",
                        c.tool.id
                    );
                }
            }
            if chip.power {
                inputs.fill(Value::known(1, 0).into());
                for vcc in b"01XE" {
                    for gnd in b"01XE" {
                        inputs[chip.index(14)] = Value::filled(1, decode(*vcc)).into();
                        inputs[chip.index(7)] = Value::filled(1, decode(*gnd)).into();
                        let out = stable.outputs(&inputs, &[])?;
                        for &[_, _, o] in chip.kind.gates() {
                            assert_eq!(
                                Value::from(out.outputs[chip.index(o)]).bit(0),
                                if *vcc == b'1' && *gnd == b'0' {
                                    decode(expected[0])
                                } else {
                                    Bit::Unknown
                                }
                            );
                        }
                    }
                }
            }
            let transition = stable.transition(&inputs, &[])?;
            assert!(
                transition.state.is_empty()
                    && !transition.outputs_changed
                    && transition.fixed_point
            );
        }
    }
    let mut registry = Registry::default();
    registry.insert(plugin.clone())?;
    networks::run(&registry);
    for definition in plugin.manifest().components() {
        for facing in Facing::ALL {
            let chip = Chip::parse(&definition.tool.id).unwrap();
            let mut c = Component::from_plugin(definition.clone(), Point::new(300, 300));
            c.facing = facing;
            let mut circuit = Circuit::default();
            for (i, p) in c.ports().iter().enumerate() {
                if !definition.tool.ports[i].output {
                    let mut v = Component::new(Kind::Constant, *p);
                    v.constant = u32::from(chip.pins()[i] == 14);
                    circuit.add_component(v);
                }
            }
            let id = circuit.add_component(c);
            let mut slow = logisim_sim::Simulator::default();
            let mut fast = logisim_sim::Simulator::default();
            for sim in [&mut slow, &mut fast] {
                sim.set_plugins(registry.clone());
                sim.synchronize(&circuit);
            }
            slow.advance(128);
            fast.settle_compiled()?;
            assert_eq!(slow.plugin_errors().count(), 0);
            assert_eq!(fast.plugin_errors().count(), 0);
            for &[_, _, out] in chip.kind.gates() {
                let i = chip.index(out);
                assert_eq!(slow.port_value(id, i), fast.port_value(id, i));
                assert!(slow.port_value(id, i).unwrap().number().is_some());
            }
            let text = logisim_file::serialize(&logisim_core::Project::single("main", circuit))?;
            assert!(logisim_file::parse(&text)?.warnings.is_empty());
            slow.set_plugins(Registry::default());
            slow.advance(128);
            assert_eq!(slow.plugin_errors().count(), 1);
            assert_eq!(
                slow.port_value(id, chip.index(chip.kind.gates()[0][2]))
                    .unwrap()
                    .bit(0),
                Bit::Error
            );
        }
    }
    println!(
        "TTL: 14 tools, four-state logic/supply matrices, event/linear execution, rotated circuits, roundtrip, bus conflicts/release, live edits, hierarchy and missing-module errors verified"
    );
    Ok(())
}
