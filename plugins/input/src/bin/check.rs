//! Exercise the actual portable artifact through both runtime transports.
use logisim_plugin::{Plugin, Registry};
use logisim_plugin_api::{
    interaction::{InputEvent, Key},
    *,
};
fn event(key: Key, pressed: bool) -> Event {
    Event::Action {
        id: "key".into(),
        argument: InputEvent::Key { key, pressed }.argument(),
    }
}
fn request(c: &Component, inputs: &[Signal], state: &[u8], event: Event) -> Evaluation {
    Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: inputs.to_vec(),
        state: state.to_vec(),
        event,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("Expected input.wasm")?;
    let plugin = Plugin::load(&std::fs::read(&path)?)?;
    assert_eq!(plugin.manifest().id, "org.logisim-rs.input");
    assert_eq!(plugin.library().interaction.keyboards.len(), 1);
    assert_eq!(
        plugin
            .library()
            .presentation
            .text("zh-CN", "Key controller"),
        "按键控制器"
    );
    let c = plugin.manifest().components().remove(0);
    let mut values: Vec<_> = c
        .tool
        .ports
        .iter()
        .map(|p| Signal {
            width: p.width,
            value: 0,
            unknown: 0,
            error: 0,
        })
        .collect();
    let mut slow = plugin.instantiate(&c)?;
    let mut fast = plugin
        .instantiate_stable(&c)?
        .ok_or("Stable contract missing")?;
    assert!(fast.uses_linear_memory());
    let reset = slow.evaluate(&c, request(&c, &values, &[], Event::Reset))?;
    let mut state = reset.state;
    let mut isolated = plugin.instantiate_stable(&c)?.unwrap();
    assert_eq!(isolated.reset(&values)?.outputs[3].value, 0);
    for (ev, held, count) in [
        (event(Key::ArrowUp, true), 1, 1),
        (event(Key::ArrowUp, true), 1, 1),
        (event(Key::ArrowLeft, true), 5, 2),
        (event(Key::ArrowUp, false), 4, 3),
    ] {
        let out = slow.evaluate(&c, request(&c, &values, &state, ev))?;
        assert_eq!(out.outputs[3].value, held);
        assert_eq!(out.outputs[8].value, count);
        assert_eq!(fast.outputs(&values, &out.state)?.outputs, out.outputs);
        state = out.state;
    }
    values[1].value = 1;
    for (clock, valid, channel, down, count) in [
        (1, 1, 2, 1, 2),
        (1, 1, 2, 1, 2),
        (0, 1, 2, 1, 2),
        (1, 1, 0, 0, 1),
        (0, 1, 0, 0, 1),
        (1, 0, 0, 0, 0),
    ] {
        values[0].value = clock;
        let out = slow.evaluate(&c, request(&c, &values, &state, Event::Propagate))?;
        let transition = fast.transition(&values, &state)?;
        assert_eq!(transition.state, out.state);
        let display = fast.outputs(&values, &transition.state)?;
        assert_eq!(display.outputs, out.outputs);
        assert_eq!(display.graphics, out.graphics);
        assert_eq!(
            (
                out.outputs[6].value,
                out.outputs[4].value,
                out.outputs[5].value,
                out.outputs[8].value
            ),
            (valid, channel, down, count)
        );
        assert!(transition.fixed_point);
        assert_eq!(
            fast.transition(&values, &transition.state)?.state,
            transition.state
        );
        state = transition.state;
    }
    let released = slow.evaluate(
        &c,
        request(
            &c,
            &values,
            &state,
            Event::Action {
                id: "key".into(),
                argument: InputEvent::ReleaseAll {}.argument(),
            },
        ),
    )?;
    assert_eq!(released.outputs[3].value, 0);
    assert_eq!(released.outputs[8].value, 0);
    assert_eq!(
        isolated.outputs(&values, &state)?,
        fast.outputs(&values, &state)?
    );
    let mut registry = Registry::default();
    registry.insert(plugin)?;
    networks(&registry, &c)?;
    if let Some(fixture) = std::env::args_os().nth(2) {
        write_fixture(&registry, &c, &path, &fixture)?;
    }
    println!(
        "Input plugin: metadata, event/linear-memory parity, queue, edges, isolation, reset, circuit routing and file roundtrip verified"
    );
    Ok(())
}
fn networks(registry: &Registry, c: &Component) -> Result<(), Box<dyn std::error::Error>> {
    use logisim_core::{Circuit, Point};
    let mut circuit = Circuit::default();
    let a = circuit.add_component(logisim_core::Component::from_plugin(
        c.clone(),
        Point::new(100, 100),
    ));
    let b = circuit.add_component(logisim_core::Component::from_plugin(
        c.clone(),
        Point::new(500, 100),
    ));
    let project = logisim_core::Project::single("Input", circuit.clone());
    let serialized = logisim_file::serialize(&project)?;
    let parsed = logisim_file::parse(&serialized)?;
    assert!(parsed.warnings.is_empty());
    assert_eq!(logisim_file::serialize(&parsed.document)?, serialized);
    let mut sim = logisim_sim::Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&circuit);
    sim.advance(256);
    assert_eq!(sim.port_value(a, 3).unwrap().number(), Some(0));
    sim.plugin_action(
        a,
        "key".into(),
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: true,
        }
        .argument(),
    );
    sim.advance(256);
    assert_eq!(sim.port_value(a, 3).unwrap().number(), Some(1));
    assert_eq!(sim.port_value(b, 3).unwrap().number(), Some(0));
    sim.settle_compiled().map_err(|e| format!("{e:?}"))?;
    assert_eq!(sim.port_value(a, 3).unwrap().number(), Some(1));
    sim.plugin_action(a, "key".into(), InputEvent::ReleaseAll {}.argument());
    sim.settle_compiled().map_err(|e| format!("{e:?}"))?;
    assert_eq!(sim.port_value(a, 3).unwrap().number(), Some(0));
    Ok(())
}
fn write_fixture(
    _registry: &Registry,
    c: &Component,
    wasm: &std::ffi::OsStr,
    path: &std::ffi::OsStr,
) -> Result<(), Box<dyn std::error::Error>> {
    use logisim_core::{Component as Part, Kind, Point, Project};
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    for x in [300, 650] {
        circuit.add_component(Part::from_plugin(c.clone(), Point::new(x, 200)));
    }
    for (y, port) in [(200, 0), (230, 1), (260, 2)] {
        let pin = Part::new(Kind::Pin, Point::new(250, y));
        circuit.add_component(pin);
        circuit.add_wire(Point::new(250, y), Point::new(300, 200 + 30 * port));
    }
    // The fixture is deliberately local; its dependency points at the exact checked artifact.
    project
        .plugin_sources
        .push(logisim_core::plugin::PluginSource {
            plugin: c.plugin.clone(),
            version: c.version.clone(),
            path: std::fs::canonicalize(wasm)?.to_string_lossy().into(),
            fallback: String::new(),
        });
    let text = logisim_file::serialize(&project)?;
    let parsed = logisim_file::parse(&text)?;
    assert!(parsed.warnings.is_empty());
    assert_eq!(logisim_file::serialize(&parsed.document)?, text);
    std::fs::write(path, text)?;
    Ok(())
}
