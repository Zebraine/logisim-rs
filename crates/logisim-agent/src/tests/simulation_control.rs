use super::{Temp, open};
use crate::{Request, Result, Service};
use logisim_core::{Circuit, Component, Gate, Kind, Point, Project};
use serde_json::{Value, json};
use std::fs;

struct Session {
    service: Service,
    id: String,
    temp: Temp,
}
impl Session {
    fn new(project: &Project) -> Self {
        let temp = Temp::new();
        let path = temp.0.join("test.circ");
        fs::write(&path, logisim_file::serialize(project).unwrap()).unwrap();
        let mut service = Service::default();
        let id = open(&mut service, &path);
        Self { service, id, temp }
    }
    fn call(&mut self, tool: &str, mut args: Value) -> Result<Value> {
        args["session"] = self.id.clone().into();
        self.service
            .call(Request::from_json(json!({"tool":tool, "arguments":args}))?)
    }
    fn state(&mut self) -> Value {
        self.call("simulation_state", json!({})).unwrap()
    }
    fn act(&mut self, tool: &str, args: Value) -> Value {
        let result = self.call(tool, args).unwrap();
        assert_eq!(result["state"], self.state());
        result
    }
}
fn component(kind: Kind, x: i32, y: i32) -> Component {
    Component::new(kind, Point::new(x, y))
}

#[test]
fn paused_input_single_batch_and_clock_edges_respect_both_engines() {
    let mut circuit = Circuit::default();
    circuit.add_component(component(Kind::Pin, 0, 0));
    circuit.add_component(component(Kind::Gate(Gate::Not), 20, 0));
    circuit.add_component(component(Kind::Gate(Gate::Not), 40, 0));
    circuit.add_component(component(Kind::Led, 40, 0));
    circuit.add_component(component(Kind::Clock, 100, 0));
    let project = Project::single("main", circuit);
    for mode in ["event", "stable", "auto"] {
        let mut s = Session::new(&project);
        let base = s.call("simulation_open", json!({"mode":mode})).unwrap();
        assert_eq!(
            base["engine"],
            if mode == "event" { "event" } else { "stable" }
        );
        let paused = s.act("simulation_set_enabled", json!({"enabled":false}));
        assert_eq!(base["steps"], paused["state"]["steps"]);
        let stimulus = s.act("simulation_set_input", json!({"id":1,"value":1}));
        assert!(stimulus["state"]["pending_count"].as_u64().unwrap() > 0);
        assert_eq!(stimulus["state"]["steps"], base["steps"]);
        let mid = s.act("simulation_step", json!({}));
        assert_eq!(mid["changed"], true);
        assert_eq!(
            mid["state"]["steps"].as_u64().unwrap(),
            base["steps"].as_u64().unwrap() + 1
        );
        assert!(mid["state"]["pending_count"].as_u64().unwrap() > 0);
        assert_eq!(
            s.call("simulation_tick", json!({})).unwrap_err().code,
            "not_settled"
        );
        assert_eq!(s.state(), mid["state"]);
        let settled = s.act("simulation_settle", json!({}));
        assert_eq!(settled["state"]["pending_count"], 0);
        assert_eq!(settled["state"]["components"][3]["tone"], "high");
        assert_eq!(settled["state"]["ticks"], 0);
        assert_eq!(s.act("simulation_step", json!({}))["changed"], false);
        assert_eq!(s.act("simulation_settle", json!({}))["changed"], false);
        for n in 1..=2 {
            let tick = s.act("simulation_tick", json!({}));
            assert_eq!(tick["state"]["ticks"], n);
            assert_eq!(tick["state"]["pending_count"], 0);
        }
        s.act("simulation_set_input", json!({"id":1,"value":0}));
        let resumed = s.act("simulation_set_enabled", json!({"enabled":true}));
        assert_eq!(resumed["state"]["components"][3]["tone"], "low");
        assert_eq!(resumed["state"]["ticks"], 2);
        let reopened = s.call("simulation_open", json!({})).unwrap();
        assert_eq!(reopened["created"], false);
        assert_eq!(reopened["steps"], resumed["state"]["steps"]);
        let reset = s.act("simulation_reset", json!({}));
        assert_eq!(reset["state"]["ticks"], 0);
        assert_eq!(reset["state"]["steps"], 0);
        assert_eq!(reset["state"]["pending_count"], 0);
    }
}

#[test]
fn only_supported_root_inputs_accept_their_actual_control_width_and_unknown() {
    let mut c = Circuit::default();
    let mut pin = component(Kind::Pin, 0, 0);
    pin.pin.three_state = true;
    pin.bits = 32;
    c.add_component(pin);
    c.add_component(component(Kind::Switch, 100, 0));
    c.add_component(component(Kind::DipSwitch, 200, 0));
    c.add_component(component(Kind::Button, 300, 0));
    let mut out = component(Kind::Pin, 400, 0);
    out.output = true;
    c.add_component(out);
    c.add_component(component(Kind::Probe, 500, 0));
    let mut s = Session::new(&Project::single("main", c));
    let old = fs::read(s.temp.0.join("test.circ")).unwrap();
    s.call("simulation_open", json!({})).unwrap();
    let set = s.act("simulation_set_input", json!({"id":1,"value":u32::MAX}));
    assert_eq!(set["state"]["pins"][0]["value"]["number"], u32::MAX);
    let unknown = s.act("simulation_set_input", json!({"id":1,"unknown":true}));
    assert_eq!(unknown["state"]["inputs"][0]["tone"], "unknown");
    assert_eq!(unknown["state"]["inputs"][0]["value"]["bits"], 32);
    s.act("simulation_set_input", json!({"id":2,"value":1}));
    let dip = s.act("simulation_set_input", json!({"id":3,"value":15}));
    assert_eq!(dip["state"]["inputs"][2]["value"]["number"], 15);
    for args in [
        json!({"id":2,"value":2}),
        json!({"id":2,"unknown":true}),
        json!({"id":3,"value":16}),
        json!({"id":4,"value":0}),
        json!({"id":5,"value":0}),
        json!({"id":6,"value":0}),
        json!({"id":99,"value":0}),
    ] {
        assert_eq!(
            s.call("simulation_set_input", args).unwrap_err().code,
            "invalid_input"
        );
    }
    assert_eq!(s.state(), dip["state"]);
    let tick = s.act("simulation_tick", json!({}));
    assert_eq!(tick["reason"], "no_clocks");
    assert_eq!(tick["changed"], false);
    assert_eq!(tick["state"], dip["state"]);
    assert_eq!(fs::read(s.temp.0.join("test.circ")).unwrap(), old);
}

#[test]
fn actual_clock_sources_are_scoped_to_instantiated_definitions() {
    let mut p = Project::default();
    let mut matrix = component(Kind::DotMatrix, 0, 0);
    matrix.io.matrix_persistence = 2;
    p.main_circuit_mut().circuit.add_component(matrix);
    let child = p.add("child".into()).unwrap();
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(component(Kind::Clock, 0, 0));
    let mut s = Session::new(&p);
    s.call("simulation_open", json!({})).unwrap();
    assert_eq!(s.act("simulation_tick", json!({}))["reason"], "no_clocks");
    let instance = p.instance(child, Point::new(200, 100)).unwrap();
    p.main_circuit_mut().circuit.add_component(instance);
    let mut nested = Session::new(&p);
    nested.call("simulation_open", json!({})).unwrap();
    assert_eq!(
        nested.act("simulation_tick", json!({}))["state"]["ticks"],
        1
    );
}

#[test]
fn event_fallback_reports_its_reason_and_stops_an_oscillating_network() {
    let mut c = Circuit::default();
    let gate = component(Kind::Gate(Gate::Nor), 200, 100);
    let ports = gate.ports();
    c.add_component(gate);
    c.add_component(Component::new(Kind::Pin, ports[2]));
    c.add_route(ports[0], ports[1], false);
    let mut s = Session::new(&Project::single("main", c));
    let initial = s.call("simulation_open", json!({"mode":"stable"})).unwrap();
    assert_eq!(initial["engine"], "event");
    assert!(
        initial["fallback_reason"]
            .as_str()
            .unwrap()
            .contains("Feedback")
    );
    let pin = &initial["inputs"][0]["id"];
    s.act("simulation_set_input", json!({"id":pin,"value":1}));
    let error = s
        .call("simulation_set_input", json!({"id":pin,"value":0}))
        .unwrap_err();
    assert_eq!(error.code, "invalid_state");
    let stopped = s.state();
    assert_eq!(error.details["state"], stopped);
    assert_eq!(stopped["limited"], true);
    assert_eq!(stopped["enabled"], false);
    for action in ["simulation_step", "simulation_tick", "simulation_settle"] {
        assert_eq!(s.call(action, json!({})).unwrap_err().code, "invalid_state");
    }
    assert_eq!(s.state(), stopped);
    assert_eq!(
        s.act("simulation_reset", json!({}))["state"]["limited"],
        false
    );
}

#[test]
fn running_state_is_per_session_and_survives_only_nonchanging_project_actions() {
    let mut c = Circuit::default();
    c.add_component(component(Kind::Clock, 0, 0));
    let mut s = Session::new(&Project::single("main", c));
    let path = s.temp.0.join("test.circ");
    let other = open(&mut s.service, &path);
    s.call("simulation_open", json!({})).unwrap();
    let changed = s.act("simulation_tick", json!({}))["state"].clone();
    let sibling = s
        .service
        .call(Request::SimulationOpen {
            session: other,
            mode: Default::default(),
        })
        .unwrap();
    assert_eq!(sibling["ticks"], 0);
    s.call("project_save", json!({"backup":false})).unwrap();
    let ops = json!([{"op":"add_circuit","name":"child"}]);
    s.call("project_patch", json!({"dry_run":true,"ops":ops}))
        .unwrap();
    s.call("project_patch", json!({"dry_run":false,"ops":[]}))
        .unwrap();
    assert_eq!(s.state(), changed);
    s.call("project_patch", json!({"dry_run":false,"ops":ops}))
        .unwrap();
    assert_eq!(
        s.call("simulation_tick", json!({})).unwrap_err().code,
        "simulation_not_open"
    );
    let reopened = s.call("simulation_open", json!({})).unwrap();
    assert_eq!(reopened["created"], true);
    assert_eq!(reopened["ticks"], 0);
    s.call("project_close", json!({})).unwrap();
    assert_eq!(
        s.call("simulation_state", json!({})).unwrap_err().code,
        "unknown_session"
    );
}

#[test]
fn hierarchy_initialization_error_preserves_requested_mode_and_rejects_controls() {
    // This is a valid document, but its active instance path exceeds the simulator's depth limit.
    let mut project = Project::default();
    let mut parent = project.main;
    for depth in 1..=34 {
        let child = project.add(format!("level-{depth}")).unwrap();
        let instance = project.instance(child, Point::ZERO).unwrap();
        project
            .circuit_mut(parent)
            .unwrap()
            .circuit
            .add_component(instance);
        parent = child;
    }
    let mut session = Session::new(&project);
    let initial = session
        .call("simulation_open", json!({"mode":"event"}))
        .unwrap();
    assert_eq!(initial["engine"], "event");
    assert!(initial["error"].as_str().unwrap().contains("Hierarchy"));
    assert_eq!(initial["enabled"], false);
    for action in [
        "simulation_step",
        "simulation_tick",
        "simulation_settle",
        "simulation_reset",
    ] {
        assert_eq!(
            session.call(action, json!({})).unwrap_err().code,
            "invalid_state"
        );
    }
    assert_eq!(session.state()["ticks"], 0);
}
