use super::*;
use crate::{ProjectSimulator, combinational::BoundaryReason, stable::Failure};
use logisim_core::{Component, FlipFlop, Gate, Point, Project};
use logisim_plugin::Plugin;

/// A standalone Wasm test tool with a conservative input dependency, explicit
/// state and optional faults. No native callback stands in for plugin execution.
fn module(id: &str, abi: u32, contract: Option<api::StableContract>, fault: &str) -> Plugin {
    use api::*;
    let manifest = Manifest {
        abi,
        id: id.into(),
        version: "1".into(),
        name: "Test plugin".into(),
        tools: vec![Tool {
            id: "test".into(),
            name: "Test".into(),
            bounds: [[-30, -10], [0, 10]],
            ports: vec![
                Port {
                    id: "out".into(),
                    label: "Output".into(),
                    at: [0, 0],
                    width: 1,
                    output: true,
                },
                Port {
                    id: "in".into(),
                    label: "Input".into(),
                    at: [-30, 0],
                    width: 1,
                    output: false,
                },
            ],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    };
    let outputs = vec![Value::known(1, 1).into(), Value::known(1, 0).into()];
    let sample = |state| {
        if abi >= 3 {
            Response::StableTransition(StateTransition {
                state,
                outputs_changed: matches!(fault, "conservative" | "output" | "raster"),
                fixed_point: !matches!(fault, "conservative" | "resample" | "oscillate"),
            })
        } else {
            Response::StableState(state)
        }
    };
    let responses = [
        Response::Manifest(manifest),
        Response::StableContract(contract),
        Response::Update(Update {
            outputs: outputs.clone(),
            state: vec![],
            graphics: vec![],
        }),
        Response::StableOutputs(StableOutputs {
            outputs,
            graphics: vec![],
        }),
        if fault == "sample" {
            Response::Error("sample failed".into())
        } else {
            sample(vec![1])
        },
        sample(vec![0]),
    ];
    let mut data = String::new();
    let mut offsets = vec![];
    for (i, response) in responses.into_iter().enumerate() {
        let bytes = serde_json::to_vec(&response).unwrap();
        let offset = 262144 + i * 32768;
        data.push_str(&format!(
            "(data (i32.const {offset}) \"{}\")",
            bytes
                .iter()
                .map(|b| format!("\\{b:02x}"))
                .collect::<String>()
        ));
        offsets.push(((bytes.len() as u64) << 32) | offset as u64);
    }
    let [manifest, contract, reset, output, sample, zero] = offsets.as_slice() else {
        unreachable!()
    };
    let output_code = if fault == "second-output" {
        "global.get $outputs if unreachable end i32.const 1 global.set $outputs"
    } else {
        ""
    };
    let sample_code = if fault == "oscillate" {
        format!(
            "local.get $n i32.const 4 i32.sub i32.load8_u i32.const 49 i32.eq if (result i64) i64.const {zero} else i64.const {sample} end"
        )
    } else {
        format!("i64.const {sample}")
    };
    let raster = if fault == "raster" {
        r#"(data (i32.const 200000) "LSR1")
        (global $raster (mut i32) (i32.const 0))
        (func (export "logisim_raster_version") (result i32) i32.const 1)
        (func (export "logisim_raster_config") (param i32) (result i32) i32.const 0)
        (func (export "logisim_raster_call") (param i32) (result i64)
            global.get $raster if unreachable end
            i32.const 1 global.set $raster
            i64.const 171798891840)"#
    } else {
        ""
    };
    let bytes = wat::parse_str(format!(
        r#"(module (memory (export "memory") 8)
      (global $outputs (mut i32) (i32.const 0)) {data} {raster}
      (func (export "logisim_abi") (result i32) i32.const {abi})
      (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
      (func (export "logisim_call") (param $n i32) (result i64)
        i32.const 9 i32.load8_u i32.const 100 i32.eq
        if (result i64) i64.const {manifest}
        else i32.const 9 i32.load8_u i32.const 101 i32.eq
          if (result i64) i64.const {reset}
          else i32.const 16 i32.load8_u i32.const 99 i32.eq
            if (result i64) i64.const {contract}
            else i32.const 16 i32.load8_u i32.const 111 i32.eq
              if (result i64) {output_code} i64.const {output}
              else {sample_code} end
            end
          end
        end))"#
    ))
    .unwrap();
    Plugin::load(&bytes).unwrap()
}
fn contract(combinational: bool, state: bool) -> Option<api::StableContract> {
    Some(api::StableContract {
        outputs: vec![api::OutputDependency {
            port: 0,
            inputs: if combinational { vec![1] } else { vec![] },
        }],
        graphics_inputs: vec![],
        state_inputs: state.then_some(vec![1]),
    })
}
fn library(c: &mut Circuit, registry: &mut Registry, plugin: Plugin) -> Id {
    let definition = plugin.manifest().components().remove(0);
    registry.insert(plugin).unwrap();
    c.add_component(Component::from_plugin(
        definition,
        Point::new(100 + c.components.len() as i32 * 200, 100),
    ))
}
fn add(c: &mut Circuit, kind: Kind) -> Id {
    c.add_component(Component::new(
        kind,
        Point::new(100 + c.components.len() as i32 * 200, 300),
    ))
}
fn link(c: &mut Circuit, a: Port, b: Port) {
    for port in [a, b] {
        let mut t = Component::new(Kind::Tunnel, c.component(port.0).unwrap().ports()[port.1]);
        t.bits = c.component(port.0).unwrap().port_width(port.1).unwrap();
        t.label = format!("{}:{}", a.0, a.1);
        c.add_component(t);
    }
}
#[test]
fn plugin_feedback_is_checked_through_expressions_but_state_feedback_is_allowed() {
    for combinational in [false, true] {
        let mut c = Circuit::default();
        let mut registry = Registry::default();
        let p = library(
            &mut c,
            &mut registry,
            module("test", 2, contract(combinational, true), ""),
        );
        let gate = add(&mut c, Kind::Gate(Gate::Buffer));
        link(&mut c, (p, 0), (gate, 1));
        link(&mut c, (gate, 0), (p, 1));
        let mut sim = Simulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&c);
        let before = (
            sim.steps(),
            sim.pending_count(),
            sim.plugin_states[&p].state.clone(),
        );
        if combinational {
            assert!(matches!(
                sim.settle_compiled().unwrap_err().reason,
                Failure::Topology {
                    reason: BoundaryReason::Feedback,
                    ..
                }
            ));
            assert_eq!(
                before,
                (
                    sim.steps(),
                    sim.pending_count(),
                    sim.plugin_states[&p].state.clone()
                )
            );
        } else {
            let report = sim.settle_compiled().unwrap();
            assert!(report.sampled_plugins > 0);
            assert_eq!(sim.port_value(gate, 0), Some(Value::known(1, 1)));
            assert_eq!(sim.plugin_states[&p].state, vec![1]);
            let report = sim.settle_compiled().unwrap();
            assert_eq!((report.evaluated_plugins, report.sampled_plugins), (0, 0));
        }
    }
}
#[test]
fn missing_capabilities_reject_atomically_and_module_replacement_recovers() {
    for abi in [1, 2] {
        let mut c = Circuit::default();
        let mut registry = Registry::default();
        let p = library(&mut c, &mut registry, module("test", abi, None, ""));
        let mut sim = Simulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&c);
        let before = (sim.steps(), sim.pending_count());
        assert_eq!(
            sim.settle_compiled().unwrap_err().reason,
            Failure::UnsupportedComponent(p)
        );
        assert_eq!(before, (sim.steps(), sim.pending_count()));
        sim.advance(32);
        assert_eq!(sim.port_value(p, 0), Some(Value::known(1, 1)));
        let mut replacement = Registry::default();
        replacement
            .insert(module("test", 2, contract(false, false), ""))
            .unwrap();
        sim.set_plugins(replacement);
        sim.settle_compiled().unwrap();
        sim.set_plugins(Registry::default());
        assert!(
            matches!(sim.settle_compiled().unwrap_err().reason,Failure::Plugin {component,..} if component==p)
        );
    }
}
fn child(p: &mut Project, registry: &mut Registry, plugin: Plugin) -> (Id, Id, Id, Id) {
    let definition = p.add(plugin.manifest().id.clone()).unwrap();
    let c = &mut p.circuit_mut(definition).unwrap().circuit;
    let id = library(c, registry, plugin);
    let input = add(c, Kind::Pin);
    let output = add(c, Kind::Pin);
    c.components
        .iter_mut()
        .find(|c| c.id == output)
        .unwrap()
        .output = true;
    link(c, (input, 0), (id, 1));
    link(c, (id, 0), (output, 0));
    (definition, id, input, output)
}
#[test]
fn hierarchical_feedback_check_is_invalidated_when_only_module_capabilities_change() {
    let mut p = Project::default();
    let mut registry = Registry::default();
    let (child, _, input, output) = child(
        &mut p,
        &mut registry,
        module("test", 2, contract(false, true), ""),
    );
    let c = p.instance(child, Point::new(100, 100)).unwrap();
    let i = c
        .interface
        .ports
        .iter()
        .position(|p| p.pin == input)
        .unwrap();
    let o = c
        .interface
        .ports
        .iter()
        .position(|p| p.pin == output)
        .unwrap();
    let id = p.main_circuit_mut().circuit.add_component(c);
    link(&mut p.main_circuit_mut().circuit, (id, o), (id, i));
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry);
    sim.synchronize(&p, p.main);
    sim.settle_compiled().unwrap();
    let mut replacement = Registry::default();
    replacement
        .insert(module("test", 2, contract(true, true), ""))
        .unwrap();
    sim.set_plugins(replacement);
    let before = (sim.steps(), sim.pending_count());
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
    assert_eq!(before, (sim.steps(), sim.pending_count()));
}
#[test]
fn failed_sample_discards_all_staged_instances_before_builtin_sampling() {
    for abi in [2, 3] {
        let mut p = Project::default();
        let mut registry = Registry::default();
        let (good, g, _, _) = child(
            &mut p,
            &mut registry,
            module("good", abi, contract(false, true), ""),
        );
        let (bad, b, _, _) = child(
            &mut p,
            &mut registry,
            module("bad", abi, contract(false, true), "sample"),
        );
        let good = p.instance(good, Point::new(100, 100)).unwrap();
        let bad = p.instance(bad, Point::new(600, 100)).unwrap();
        let c = &mut p.main_circuit_mut().circuit;
        let good = c.add_component(good);
        let bad = c.add_component(bad);
        let ff = add(c, Kind::FlipFlop(FlipFlop::D));
        let clock = add(c, Kind::Pin);
        link(c, (clock, 0), (ff, 2));
        let data = add(c, Kind::Pin);
        link(c, (data, 0), (ff, 3));
        let ram = add(c, Kind::Ram);
        let memory = c.components.iter_mut().find(|c| c.id == ram).unwrap();
        memory.bits = 1;
        memory.memory.interface = logisim_core::MemoryInterface::Separate;
        memory.memory.address_bits = 2;
        let data_port = memory.memory_port(logisim_core::MemoryRole::Input).unwrap();
        let clock_port = memory.memory_port(logisim_core::MemoryRole::Clock).unwrap();
        link(c, (data, 0), (ram, data_port));
        link(c, (clock, 0), (ram, clock_port));
        let address = add(c, Kind::Constant);
        let constant = c.components.iter_mut().find(|c| c.id == address).unwrap();
        constant.bits = 2;
        constant.constant = 0;
        link(c, (address, 0), (ram, 1));
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&p, p.main);
        sim.advance(128);
        sim.set_input(data, 1);
        sim.set_input(clock, 1);
        let error = sim.settle_compiled().unwrap_err();
        assert_eq!(error.path, vec![bad]);
        assert_eq!(
            sim.view(&[]).unwrap().memory_contents(ram).unwrap().get(0),
            0
        );
        assert!(matches!(error.reason,Failure::Plugin {component,..} if component==b));
        assert!(
            sim.view(&[good]).unwrap().plugin_states[&g]
                .state
                .is_empty()
        );
        assert_eq!(
            sim.view(&[]).unwrap().stored_value(ff),
            Some(Value::known(1, 0))
        );
        assert_eq!(
            sim.view(&[bad]).unwrap().port_value(b, 0),
            Some(Value::filled(1, Bit::Error))
        );
        assert!(
            sim.view(&[bad])
                .unwrap()
                .plugin_graphics(b)
                .unwrap()
                .is_empty()
        );
        assert!(sim.settle_compiled().is_err());
    }
}
#[test]
fn output_fault_is_visible_and_reset_recreates_the_phase_vm() {
    let mut c = Circuit::default();
    let mut registry = Registry::default();
    let id = library(
        &mut c,
        &mut registry,
        module("test", 2, contract(true, false), "second-output"),
    );
    let pin = add(&mut c, Kind::Pin);
    link(&mut c, (pin, 0), (id, 1));
    let mut sim = Simulator::default();
    sim.set_plugins(registry);
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    sim.set_input(pin, 1);
    assert!(
        matches!(sim.settle_compiled().unwrap_err().reason,Failure::Plugin {component,..} if component==id)
    );
    assert_eq!(sim.port_value(id, 0), Some(Value::filled(1, Bit::Error)));
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(id, 0), Some(Value::known(1, 1)));
}
#[test]
fn changing_state_without_changing_drives_is_bounded() {
    for abi in [2, 3] {
        let mut c = Circuit::default();
        let mut registry = Registry::default();
        library(
            &mut c,
            &mut registry,
            module("test", abi, contract(false, true), "oscillate"),
        );
        let mut sim = Simulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&c);
        assert_eq!(
            sim.settle_compiled().unwrap_err().reason,
            Failure::PropagationLimit
        );
        assert!(sim.limited());
        assert!(!sim.enabled());
    }
}

#[test]
fn only_explicit_transition_promises_skip_output_and_sample_rechecks() {
    for (abi, mode, samples, outputs) in [
        (2, "", 2, 2),
        (3, "conservative", 2, 2),
        (3, "", 1, 1),
        (3, "output", 1, 2),
        (3, "resample", 2, 1),
    ] {
        let mut c = Circuit::default();
        let mut registry = Registry::default();
        let id = library(
            &mut c,
            &mut registry,
            module("test", abi, contract(false, true), mode),
        );
        let input = add(&mut c, Kind::Pin);
        link(&mut c, (input, 0), (id, 1));
        let mut sim = Simulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&c);
        let report = sim.settle_compiled().unwrap();
        assert_eq!(report.sampled_plugins, samples);
        assert_eq!(report.evaluated_plugins, outputs);
        assert_eq!(sim.plugin_states[&id].state, [1]);
        assert!(sim.plugin_states[&id].event_dirty);
        let report = sim.settle_compiled().unwrap();
        assert_eq!(report.sampled_plugins, 0);
        assert_eq!(report.evaluated_plugins, 0);
        sim.set_input(input, 1);
        let report = sim.settle_compiled().unwrap();
        assert_eq!(report.sampled_plugins, 1);
        assert_eq!(report.evaluated_plugins, 0);
        sim.reset();
        let report = sim.settle_compiled().unwrap();
        assert_eq!(report.sampled_plugins, samples);
        assert_eq!(report.evaluated_plugins, outputs);
    }
}

#[test]
fn cooperative_batches_keep_state_only_work_pending_and_bound_convergence() {
    for mode in ["conservative", "oscillate"] {
        let mut p = Project::default();
        let mut registry = Registry::default();
        library(
            &mut p.main_circuit_mut().circuit,
            &mut registry,
            module("test", 3, contract(false, true), mode),
        );
        p.main_circuit_mut()
            .circuit
            .add_component(Component::new(Kind::Clock, Point::ZERO));
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(registry);
        sim.synchronize(&p, p.main);
        let before = (sim.steps(), sim.pending_count());
        sim.prepare_compiled().unwrap();
        assert_eq!(before, (sim.steps(), sim.pending_count()));
        // Reset already supplied the outputs: sampling changes state without a new drive.
        sim.step_compiled().unwrap();
        assert_eq!(sim.view(&[]).unwrap().pending_count(), 0);
        assert!(sim.pending_count() > 0);
        assert!(!sim.tick());
        if mode == "oscillate" {
            for _ in 1..crate::PROPAGATION_LIMIT - 1 {
                sim.step_compiled().unwrap();
                assert!(sim.pending_count() > 0);
            }
            assert_eq!(
                sim.step_compiled().unwrap_err().reason,
                Failure::PropagationLimit
            );
            assert!(sim.limited());
            assert_eq!(sim.steps(), crate::PROPAGATION_LIMIT as u64);
        } else {
            let before = sim.steps();
            sim.set_enabled(false);
            assert!(sim.step()); // Hand off even though there were no pending drives.
            assert_eq!(sim.steps(), before + 1);
            while sim.step() {}
            assert_eq!(sim.pending_count(), 0);
            sim.settle_compiled().unwrap();
            assert_eq!(sim.pending_count(), 0);
        }
        sim.reset();
        assert!(!sim.limited());
        assert!(sim.prepare_compiled().is_ok());
    }
}

#[test]
fn raster_fault_after_state_commit_publishes_error_and_stops_stable_execution() {
    let mut c = Circuit::default();
    let mut registry = Registry::default();
    let p = library(
        &mut c,
        &mut registry,
        module("raster.test", 3, contract(false, true), "raster"),
    );
    let mut sim = Simulator::default();
    sim.set_plugins(registry);
    sim.synchronize(&c);
    assert_eq!(sim.plugin_errors().count(), 0);
    assert!(
        matches!(sim.settle_compiled().unwrap_err().reason, Failure::Plugin { component, .. } if component == p)
    );
    assert_eq!(sim.port_value(p, 0), Some(Value::filled(1, Bit::Error)));
    assert!(sim.plugin_raster(p).is_none());
    assert_eq!(sim.plugin_errors().count(), 1);
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Plugin { .. }
    ));
}
