use logisim_core::{Bit, Value};
use logisim_plugin::{Registry, StableInstance};
use logisim_plugin_api::*;

fn inputs() -> Vec<Signal> {
    [
        (32, 0),
        (32, 0),
        (32, 42),
        (1, 0),
        (1, 1),
        (5, 1),
        (5, 1),
        (5, 0),
    ]
    .into_iter()
    .map(|(w, v)| Value::known(w, v).into())
    .collect()
}
fn vm(registry: &Registry, tool: &Component) -> Result<StableInstance, String> {
    registry
        .instantiate_stable(tool)?
        .ok_or("Missing stable-state capability".into())
}
pub fn run(registry: &Registry, tools: &[Component]) -> Result<(), String> {
    let c = &tools[0];
    let mut a = vm(registry, c)?;
    assert_eq!(
        a.contract().outputs,
        vec![
            OutputDependency {
                port: 0,
                inputs: vec![6]
            },
            OutputDependency {
                port: 1,
                inputs: vec![7]
            },
        ]
    );
    assert_eq!(a.contract().state_inputs, Some(vec![2, 3, 4, 5]));
    assert!(a.contract().graphics_inputs.is_empty());
    let mut b = vm(registry, c)?;
    let mut pa = inputs();
    let mut pb = pa.clone();
    pb[2].value = 0;
    let mut sa = a.reset(&pa)?.state;
    let mut sb = b.reset(&pb)?.state;
    // Snapshots produced by the old JSON codec remain valid in the actual VM.
    let mut legacy = vec![Signal::from(Value::known(32, 0)); 32];
    legacy.push(Value::known(1, 0).into());
    let legacy = serde_json::to_vec(&legacy).map_err(|e| e.to_string())?;
    assert_eq!(a.outputs(&pa, &legacy)?, a.outputs(&pa, &sa)?);
    pa[3].value = 1;
    assert_eq!(a.sample(&pa, &legacy)?, a.sample(&pa, &sa)?);
    pa[3].value = 0;
    // Two output closures and one simultaneous sample per external edge. Q2 sees
    // old Q1 even when Q1's next-state computation happens first.
    for (clock, expected) in [(1, [42, 0]), (0, [42, 0]), (1, [42, 42])] {
        pa[3].value = clock;
        pb[3].value = clock;
        let before = a.outputs(&pa, &sa)?;
        assert_eq!(a.outputs(&pa, &sa)?, before);
        pb[2] = before.outputs[0];
        let next_a = a.sample(&pa, &sa)?;
        let next_b = b.sample(&pb, &sb)?;
        // Sampling hasn't published or modified the explicit old state.
        assert_eq!(a.outputs(&pa, &sa)?, before);
        sa = next_a;
        sb = next_b;
        assert_eq!(a.outputs(&pa, &sa)?.outputs[0].value, expected[0]);
        assert_eq!(b.outputs(&pb, &sb)?.outputs[0].value, expected[1]);
        assert_eq!(a.sample(&pa, &sa)?, sa);
        assert_eq!(b.sample(&pb, &sb)?, sb);
    }
    assert_eq!(a.reset(&pa)?.outputs[0].value, 0);
    assert_eq!(b.outputs(&pb, &sb)?.outputs[0].value, 42);
    // Same explicit state is portable between the event VM and the stable VM.
    let mut events = registry.instantiate(c)?;
    let edited = events.evaluate(
        c,
        Evaluation {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
            inputs: pa.clone(),
            state: sa,
            event: Event::Action {
                id: "write".into(),
                argument: "1 DEADBEEF".into(),
            },
        },
    )?;
    assert_eq!(a.outputs(&pa, &edited.state)?.outputs, edited.outputs);
    assert_eq!(a.outputs(&pa, &edited.state)?.graphics, edited.graphics);
    // Read addresses can change outputs without sampling or changing stored words.
    pa[6].value = 0;
    assert_eq!(a.outputs(&pa, &edited.state)?.outputs[0].value, 0);
    pa[6].value = 1;
    assert_eq!(a.outputs(&pa, &edited.state)?.outputs[0].value, 0xdeadbeef);
    assert!(a.outputs(&pa, b"invalid state").is_err());
    assert!(a.sample(&pa, &edited.state).is_err()); // sticky fault
    let cleared = a.reset(&pa)?;
    assert_eq!(a.outputs(&pa, &cleared.state)?.outputs[0].value, 0);
    for trigger in ["high", "low"] {
        let mut c = c.clone();
        c.properties.insert("trigger".into(), trigger.into());
        assert!(registry.instantiate_stable(&c)?.is_none());
        registry.instantiate(&c)?; // still available to the event engine
    }
    four_state(registry, c)?;
    let mut probe = vm(registry, &tools[1])?;
    assert!(probe.contract().state_inputs.is_none());
    assert!(probe.contract().outputs.is_empty());
    assert_eq!(probe.contract().graphics_inputs, vec![0]);
    let p = [Value::known(32, 0x800211f3).into()];
    let reset = probe.reset(&p)?;
    assert!(reset.state.is_empty());
    assert_eq!(probe.outputs(&p, &[])?.graphics, reset.graphics);
    assert!(matches!(&reset.graphics[0], Shape::Text{text,..} if text=="csrrw gp,2048,tp"));
    assert!(probe.sample(&p, &[])?.is_empty());
    for bit in [Bit::Unknown, Bit::Error] {
        assert!(
            probe
                .outputs(&[Value::filled(32, bit).into()], &[])?
                .graphics
                .is_empty()
        );
    }
    println!(
        "Wasm stable phases: capabilities, simultaneous edges, immutable reads, state transport, actions, isolation, faults/reset and four-state equivalence passed"
    );
    Ok(())
}
fn four_state(registry: &Registry, component: &Component) -> Result<(), String> {
    let bits = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];
    let mut cases = 0;
    for trigger in ["rising", "falling"] {
        let mut c = component.clone();
        c.properties.insert("trigger".into(), trigger.into());
        let mut stable = vm(registry, &c)?;
        let mut event = registry.instantiate(&c)?;
        let mut p = inputs();
        p[3].value = u32::from(trigger == "falling");
        let initial = stable.reset(&p)?.state;
        for clock in bits {
            for enable in bits {
                for address in bits {
                    for data in bits {
                        let mut p = p.clone();
                        p[3] = Value::filled(1, clock).into();
                        p[4] = Value::filled(1, enable).into();
                        p[5] = Value::filled(5, address).into();
                        p[2] = Value::filled(32, data).into();
                        p[6] = Value::known(5, 31).into();
                        p[7] = Value::filled(5, address).into();
                        let old = stable.outputs(&p, &initial)?;
                        let next = stable.sample(&p, &initial)?;
                        assert_eq!(stable.outputs(&p, &initial)?, old);
                        let out = stable.outputs(&p, &next)?;
                        let legacy = event.evaluate(
                            &c,
                            Evaluation {
                                tool: c.tool.id.clone(),
                                properties: c.properties.clone(),
                                inputs: p,
                                state: initial.clone(),
                                event: Event::Propagate,
                            },
                        )?;
                        assert_eq!(next, legacy.state);
                        assert_eq!(out.outputs, legacy.outputs);
                        assert_eq!(out.graphics, legacy.graphics);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 512);
    println!("Wasm stable/event four-state cases: {cases}");
    Ok(())
}
