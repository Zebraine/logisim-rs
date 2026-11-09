//! Validate the actual Wasm artifact, including serialized instance state.
use logisim_plugin::{Plugin, Registry};
use logisim_plugin_api::{Evaluation, Event};
use logisim_ttl_sequential_plugin::{model::Chip, validation::*};
#[path = "check/networks.rs"]
mod networks;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("Expected module.wasm")?;
    let plugin = Plugin::load(&std::fs::read(path)?)?;
    assert_eq!(plugin.manifest().id, "org.logisim-rs.ttl-sequential");
    assert_eq!(plugin.manifest().tools.len(), 2);
    assert_eq!(
        plugin
            .library()
            .presentation
            .text("zh-CN", "TTL · Sequential"),
        "TTL · 时序"
    );
    let mut count = 0;
    for mut c in plugin.manifest().components() {
        let chip = Chip::parse(&c.tool.id).unwrap();
        for internal in [false, true] {
            c.properties.insert("internal".into(), internal.to_string());
            for scenario in scenarios()
                .into_iter()
                .filter(|s| chip.power || s.name != "power")
            {
                let mut event = plugin.instantiate(&c)?;
                let mut stable = plugin
                    .instantiate_stable(&c)?
                    .ok_or("Missing stable contract")?;
                assert!(stable.uses_linear_memory());
                let reset_inputs = inputs(chip, &scenario.rows[0]);
                let mut state = stable.reset(&reset_inputs)?.state;
                let mut hash = HASH_START;
                for row in &scenario.rows {
                    let values = inputs(chip, row);
                    let before = stable.outputs(&values, &state)?;
                    assert_eq!(before, stable.outputs(&values, &state)?);
                    let slow = event.evaluate(
                        &c,
                        Evaluation {
                            tool: c.tool.id.clone(),
                            properties: c.properties.clone(),
                            inputs: values.clone(),
                            state: state.clone(),
                            event: Event::Propagate,
                        },
                    )?;
                    let next = stable.transition(&values, &state)?;
                    assert_eq!(slow.state, next.state);
                    let fast = stable.outputs(&values, &next.state)?;
                    assert_eq!(slow.outputs, fast.outputs);
                    assert_eq!(slow.graphics, fast.graphics);
                    if !next.outputs_changed {
                        assert_eq!(before, fast);
                    }
                    assert_eq!(!fast.graphics.is_empty(), internal);
                    let again = stable.transition(&values, &next.state)?;
                    assert!(next.fixed_point);
                    assert_eq!(again.state, next.state);
                    assert!(!again.outputs_changed);
                    fingerprint(&mut hash, chip, &fast.outputs);
                    state = next.state;
                    count += 1;
                }
                assert_eq!(hash, scenario.expected, "{}", scenario.name);
                // A second VM must receive state explicitly; no hidden global memory.
                let last = inputs(chip, scenario.rows.last().unwrap());
                let mut fresh = plugin.instantiate_stable(&c)?.unwrap();
                assert_eq!(
                    fresh.outputs(&last, &state)?,
                    stable.outputs(&last, &state)?
                );
                assert!(fresh.outputs(&last, &[1, 2, 3]).is_err());
                let reset = fresh.reset(&reset_inputs)?;
                assert_eq!(reset.outputs, stable.reset(&reset_inputs)?.outputs);
            }
        }
    }
    let mut registry = Registry::default();
    registry.insert(plugin)?;
    networks::run(&registry);
    println!(
        "TTL sequential: {count} observed sequence frames through event and linear-memory APIs; fixed points, snapshots, reset, rotated circuits, bus release, same-edge chains, hierarchy, edits and roundtrip verified"
    );
    Ok(())
}
