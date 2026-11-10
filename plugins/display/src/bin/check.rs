//! Validate the actual artifact through event, stable, raster, circuit and file paths.
use logisim_plugin::{Plugin, Registry};
use logisim_plugin_api::*;
use std::sync::Arc;
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
    let path = std::env::args_os().nth(1).ok_or("Expected display.wasm")?;
    let plugin = Plugin::load(&std::fs::read(&path)?)?;
    assert_eq!(plugin.manifest().id, "org.logisim-rs.display");
    for c in plugin.manifest().components() {
        let mut inputs: Vec<_> = c
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
        let mut vm = plugin.instantiate(&c)?;
        let mut fast = plugin
            .instantiate_stable(&c)?
            .ok_or("Missing stable contract")?;
        assert!(fast.uses_linear_memory());
        let mut state = vm
            .evaluate(&c, request(&c, &inputs, &[], Event::Reset))?
            .state;
        let blank = vm.raster(&c, &state, None)?.ok_or("Missing raster")?;
        assert!(
            blank
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [0, 0, 0, 255])
        );
        let mut last = Arc::clone(&blank);
        // Back writes, present on next edge, and clear+write+present precedence.
        for (clock, x, y, color, write, clear, present) in [
            (1, 2, 3, 224, 1, 0, 0),
            (0, 2, 3, 224, 0, 0, 1),
            (1, 2, 3, 224, 0, 0, 1),
            (0, 0, 0, 3, 1, 1, 1),
            (1, 0, 0, 3, 1, 1, 1),
        ] {
            for (s, v) in inputs
                .iter_mut()
                .zip([clock, x, y, color, write, clear, present])
            {
                s.value = v;
            }
            let out = vm.evaluate(&c, request(&c, &inputs, &state, Event::Propagate))?;
            let next = fast.transition(&inputs, &state)?;
            assert_eq!(out.state, next.state);
            assert_eq!(fast.transition(&inputs, &next.state)?.state, next.state);
            assert!(next.fixed_point);
            let image = vm.raster(&c, &next.state, Some(&last))?.unwrap();
            assert_eq!(*fast.raster(&next.state, None)?.unwrap(), *image);
            assert_eq!(next.outputs_changed, !Arc::ptr_eq(&last, &image));
            if present == 0 {
                assert!(Arc::ptr_eq(&blank, &image));
            }
            if present == 1 && clock == 1 && clear == 0 {
                let index = (3 * image.width as usize + 2) * 4;
                assert_eq!(&image.pixels[index..index + 4], &[255, 0, 0, 255]);
            }
            if clear == 1 && clock == 1 {
                assert!(
                    image
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| *p == [0, 0, 255, 255])
                );
            }
            state = next.state;
            last = image;
        }
        // A new VM reproduces the same frame from explicit state; reset remains black.
        assert_eq!(
            *plugin.instantiate(&c)?.raster(&c, &state, None)?.unwrap(),
            *last
        );
        let reset = fast.reset(&inputs)?;
        assert_eq!(*fast.raster(&reset.state, None)?.unwrap(), *blank);
    }
    let c = plugin.manifest().components().remove(0);
    let mut registry = Registry::default();
    registry.insert(plugin)?;
    let Fixture { project, ids, a, b } = fixture(&c, &path)?;
    for stable in [false, true] {
        let mut sim = logisim_sim::Simulator::default();
        sim.set_plugins(registry.clone());
        sim.synchronize(&project.main_circuit().circuit);
        let settle = |sim: &mut logisim_sim::Simulator| -> Result<(), String> {
            if stable {
                sim.settle_compiled()
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"))
            } else {
                sim.advance(4096);
                Ok(())
            }
        };
        settle(&mut sim)?;
        let blank = sim.plugin_raster(a).ok_or("No initial raster")?.clone();
        for (id, value) in ids.iter().zip([0, 2, 3, 224, 1, 0, 0]) {
            sim.set_input(*id, value);
        }
        settle(&mut sim)?;
        sim.set_input(ids[0], 1);
        settle(&mut sim)?;
        assert!(Arc::ptr_eq(&blank, sim.plugin_raster(a).unwrap()));
        sim.set_input(ids[0], 0);
        sim.set_input(ids[6], 1);
        settle(&mut sim)?;
        sim.set_input(ids[0], 1);
        settle(&mut sim)?;
        let image = sim.plugin_raster(a).unwrap();
        let index = (3 * 16 + 2) * 4;
        assert_eq!(&image.pixels[index..index + 4], &[255, 0, 0, 255]);
        assert_eq!(sim.plugin_raster(b).unwrap().pixels, blank.pixels);
        assert_eq!(sim.plugin_errors().count(), 0);
    }
    let text = logisim_file::serialize(&project)?;
    let parsed = logisim_file::parse(&text)?;
    assert!(parsed.warnings.is_empty());
    assert_eq!(text, logisim_file::serialize(&parsed.document)?);
    if let Some(fixture) = std::env::args_os().nth(2) {
        std::fs::write(fixture, text)?;
    }
    println!(
        "Display plugin: event/linear parity, frame publication/cache, reset, isolation, network scheduling and file roundtrip verified"
    );
    Ok(())
}
struct Fixture {
    project: logisim_core::Project,
    ids: Vec<logisim_core::Id>,
    a: logisim_core::Id,
    b: logisim_core::Id,
}
fn fixture(c: &Component, wasm: &std::ffi::OsStr) -> Result<Fixture, Box<dyn std::error::Error>> {
    use logisim_core::{Component as Part, Kind, Point, Project};
    let mut p = Project::default();
    let circuit = &mut p.main_circuit_mut().circuit;
    let a = circuit.add_component(Part::from_plugin(c.clone(), Point::new(300, 200)));
    let b = circuit.add_component(Part::from_plugin(c.clone(), Point::new(650, 200)));
    let mut ids = vec![];
    for (i, port) in c.tool.ports.iter().enumerate() {
        let at = Point::new(240, 200 + 20 * i as i32);
        let mut pin = Part::new(Kind::Pin, at);
        pin.bits = port.width;
        pin.label = port.label.clone();
        ids.push(circuit.add_component(pin));
        circuit.add_wire(at, Point::new(300, at.y));
    }
    p.plugin_sources.push(logisim_core::plugin::PluginSource {
        plugin: c.plugin.clone(),
        version: c.version.clone(),
        path: std::fs::canonicalize(wasm)?.to_string_lossy().into(),
        fallback: String::new(),
    });
    Ok(Fixture {
        project: p,
        ids,
        a,
        b,
    })
}
