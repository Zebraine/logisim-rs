//! Isolated circuit instances linked by directional, four-state interface signals.
use crate::{Diagnostic, PROPAGATION_LIMIT, Problem, Simulator, Value};
use logisim_core::{Id, Kind, Project};
use std::collections::BTreeMap;

type Path = Vec<Id>;
struct Instance {
    definition: Id,
    sim: Simulator,
}
struct CachedRoot {
    states: BTreeMap<Path, Instance>,
    ticks: u64,
}
struct Bridge {
    parent: Path,
    instance: Id,
    index: usize,
    child: Path,
    pin: Id,
    output: bool,
}

pub struct ProjectSimulator {
    project: Project,
    root: Id,
    states: BTreeMap<Path, Instance>,
    bridges: Vec<Bridge>,
    cached_roots: BTreeMap<Id, CachedRoot>,
    ticks: u64,
    enabled: bool,
    steps: u64,
    unsettled: usize,
    limited: bool,
    error: Option<String>,
}
impl Default for ProjectSimulator {
    fn default() -> Self {
        Self {
            project: Project::default(),
            root: 0,
            states: BTreeMap::new(),
            bridges: vec![],
            cached_roots: BTreeMap::new(),
            ticks: 0,
            enabled: true,
            steps: 0,
            unsettled: 0,
            limited: false,
            error: None,
        }
    }
}
impl ProjectSimulator {
    pub fn synchronize(&mut self, project: &Project, root: Id) -> bool {
        if self.project == *project && self.root == root {
            return false;
        }
        let same_root = self.root == root;
        let mut old = if same_root {
            std::mem::take(&mut self.states)
        } else {
            if !self.states.is_empty() {
                self.cached_roots.insert(
                    self.root,
                    CachedRoot {
                        states: std::mem::take(&mut self.states),
                        ticks: self.ticks,
                    },
                );
            }
            let cached = self.cached_roots.remove(&root);
            self.ticks = cached.as_ref().map_or(0, |c| c.ticks);
            cached.map_or_else(BTreeMap::new, |c| c.states)
        };
        self.cached_roots
            .retain(|id, _| project.circuit(*id).is_some());
        self.project = project.clone();
        self.root = root;
        self.states.clear();
        self.bridges.clear();
        self.error = None;
        let compiled = (|| {
            project.validate()?;
            if project.circuit(root).is_none() {
                return Err("Missing simulation root.".into());
            }
            let mut stack = vec![(vec![], root)];
            let mut items = 0;
            let mut changed = !same_root;
            while let Some((path, definition)) = stack.pop() {
                if path.len() > 32 || self.states.len() >= 1024 {
                    return Err("Hierarchy exceeds 32 levels or 1024 instances.".into());
                }
                let circuit = &project
                    .circuit(definition)
                    .ok_or("Missing circuit")?
                    .circuit;
                items += circuit.components.len() + circuit.wires.len();
                if items > 65_536 {
                    return Err("Expanded hierarchy exceeds 65536 items.".into());
                }
                let mut instance = old
                    .remove(&path)
                    .filter(|s| s.definition == definition)
                    .unwrap_or_else(|| {
                        changed = true;
                        Instance {
                            definition,
                            sim: Simulator::default(),
                        }
                    });
                changed |= instance.sim.synchronize(circuit);
                for c in &circuit.components {
                    if let Kind::Subcircuit(id) = c.kind {
                        let mut child = path.clone();
                        child.push(c.id);
                        stack.push((child.clone(), id));
                        for (index, port) in c.interface.ports.iter().enumerate() {
                            self.bridges.push(Bridge {
                                parent: path.clone(),
                                instance: c.id,
                                index,
                                child: child.clone(),
                                pin: port.pin,
                                output: port.output,
                            });
                        }
                    }
                }
                self.states.insert(path, instance);
            }
            changed |= !old.is_empty();
            Ok(changed)
        })();
        match compiled {
            Ok(changed) => {
                if changed {
                    self.steps = 0;
                    self.unsettled = 0;
                    self.limited = false;
                }
                self.refresh_bridges();
                changed
            }
            Err(e) => {
                self.states.clear();
                self.bridges.clear();
                self.error = Some(e);
                true
            }
        }
    }
    fn refresh_bridges(&mut self) {
        // Gather before mutating: one deterministic boundary batch, independent of tree order.
        let values: Vec<_> = self
            .bridges
            .iter()
            .map(|b| {
                if b.output {
                    self.states[&b.child].sim.port_value(b.pin, 0)
                } else {
                    self.states[&b.parent].sim.port_value(b.instance, b.index)
                }
            })
            .collect();
        for (bridge, value) in self.bridges.iter().zip(values) {
            if let Some(value) = value {
                if bridge.output {
                    self.states
                        .get_mut(&bridge.parent)
                        .unwrap()
                        .sim
                        .drive_port((bridge.instance, bridge.index), value);
                } else {
                    self.states
                        .get_mut(&bridge.child)
                        .unwrap()
                        .sim
                        .external_input(bridge.pin, value);
                }
            }
        }
    }
    pub fn view(&self, path: &[Id]) -> Option<&Simulator> {
        self.states.get(path).map(|s| &s.sim)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn limited(&self) -> bool {
        self.limited
    }
    pub fn steps(&self) -> u64 {
        self.steps
    }
    pub fn pending_count(&self) -> usize {
        self.states.values().map(|s| s.sim.pending_count()).sum()
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.limited = false;
            self.unsettled = 0;
            for state in self.states.values_mut() {
                state.sim.set_enabled(true);
            }
        }
    }
    pub fn step(&mut self) -> bool {
        if self.limited || self.error.is_some() {
            return false;
        }
        let mut advanced = false;
        for state in self.states.values_mut() {
            advanced |= state.sim.step();
        }
        self.refresh_bridges();
        if advanced {
            self.steps = self.steps.saturating_add(1);
            self.unsettled = if self.pending_count() == 0 {
                0
            } else {
                self.unsettled + 1
            };
            if self.unsettled >= PROPAGATION_LIMIT || self.states.values().any(|s| s.sim.limited())
            {
                self.limited = true;
                self.enabled = false;
            }
        }
        advanced
    }
    pub fn advance(&mut self, budget: usize) {
        if self.enabled {
            for _ in 0..budget {
                if !self.step() {
                    break;
                }
            }
        }
    }
    pub fn reset(&mut self) {
        self.cached_roots.clear();
        self.ticks = 0;
        for state in self.states.values_mut() {
            state.sim.reset();
        }
        self.steps = 0;
        self.unsettled = 0;
        self.limited = false;
        self.refresh_bridges();
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn has_clocks(&self) -> bool {
        self.states.values().any(|s| s.sim.has_clocks())
    }
    pub fn tick(&mut self) -> bool {
        if self.limited || self.error.is_some() || self.pending_count() > 0 || !self.has_clocks() {
            return false;
        }
        for state in self.states.values_mut() {
            state.sim.tick_clocks();
        }
        self.ticks = self.ticks.saturating_add(1);
        true
    }
    pub fn set_button(&mut self, path: &[Id], id: Id, pressed: bool) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_button(id, pressed);
        }
    }
    pub fn type_character(&mut self, path: &[Id], id: Id, value: u16) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.type_character(id, value);
        }
    }
    pub fn set_joystick(&mut self, path: &[Id], id: Id, offset: logisim_core::Point) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_joystick(id, offset);
        }
    }
    pub fn release_joysticks(&mut self) {
        for state in self.states.values_mut() {
            state.sim.release_joysticks();
        }
        for root in self.cached_roots.values_mut() {
            for state in root.states.values_mut() {
                state.sim.release_joysticks();
            }
        }
    }
    pub fn release_buttons(&mut self) {
        for state in self.states.values_mut() {
            state.sim.release_buttons();
        }
        for root in self.cached_roots.values_mut() {
            for state in root.states.values_mut() {
                state.sim.release_buttons();
            }
        }
    }
    pub fn set_memory_word(&mut self, path: &[Id], id: Id, address: u32, value: u32) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.set_memory_word(id, address, value);
        }
    }
    pub fn fill_memory(&mut self, path: &[Id], id: Id, start: u32, end: u32, value: u32) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.fill_memory(id, start, end, value);
        }
    }
    pub fn clear_memory(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.clear_memory(id);
        }
    }
    pub fn replace_memory(
        &mut self,
        path: &[Id],
        id: Id,
        contents: logisim_core::MemoryContents,
    ) -> bool {
        let replaced = self
            .states
            .get_mut(path)
            .is_some_and(|state| state.sim.replace_memory(id, contents));
        if replaced {
            self.unsettled = 0;
        }
        replaced
    }
    pub fn poke_clock(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.poke_clock(id);
            self.unsettled = 0;
        }
    }
    pub fn toggle_stored(&mut self, path: &[Id], id: Id) {
        if let Some(state) = self.states.get_mut(path) {
            state.sim.toggle_stored(id);
            self.unsettled = 0;
        }
    }
    // Nested interface pins are parent-driven; local switches remain interactive.
    pub fn set_input(&mut self, id: Id, value: u32) {
        self.set_input_at(&[], id, value);
    }
    pub fn toggle_input(&mut self, id: Id, bit: u8) {
        self.toggle_input_at(&[], id, bit);
    }
    pub fn set_input_at(&mut self, path: &[Id], id: Id, value: u32) {
        if let Some(state) = self.states.get_mut(path)
            && (path.is_empty()
                || state
                    .sim
                    .circuit
                    .component(id)
                    .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch)))
        {
            state.sim.set_input(id, value);
            self.unsettled = 0;
        }
    }
    pub fn toggle_input_at(&mut self, path: &[Id], id: Id, bit: u8) {
        if let Some(state) = self.states.get_mut(path)
            && (path.is_empty()
                || state
                    .sim
                    .circuit
                    .component(id)
                    .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch)))
        {
            state.sim.toggle_input(id, bit);
            self.unsettled = 0;
        }
    }
    pub fn input(&self, id: Id) -> Option<Value> {
        self.view(&[])?.input(id)
    }
    pub fn diagnostics(&self, path: &[Id]) -> Vec<Diagnostic> {
        let mut out = vec![];
        for (child, state) in &self.states {
            if child == path {
                out.extend(state.sim.diagnostics());
            } else if child.starts_with(path)
                && let Some(instance) = child.get(path.len())
            {
                for issue in state.sim.diagnostics() {
                    out.push(Diagnostic {
                        problem: issue.problem,
                        components: vec![*instance],
                        wires: vec![],
                    });
                }
            }
        }
        if self.limited {
            out.push(Diagnostic {
                problem: Problem::PropagationLimit,
                components: vec![],
                wires: vec![],
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bit;
    use logisim_core::{Component, Facing, Gate, Point};
    fn inverter(p: &mut Project, name: &str, bits: u8) -> (Id, Id, Id) {
        let id = p.add(name.into()).unwrap();
        let c = &mut p.circuit_mut(id).unwrap().circuit;
        let mut input = Component::new(Kind::Pin, Point::new(100, 100));
        input.bits = bits;
        let input = c.add_component(input);
        let mut gate = Component::new(Kind::Gate(Gate::Not), Point::new(180, 100));
        gate.bits = bits;
        c.add_component(gate);
        let mut output = Component::new(Kind::Pin, Point::new(240, 100));
        output.bits = bits;
        output.output = true;
        output.facing = Facing::West;
        let output = c.add_component(output);
        c.add_wire(Point::new(100, 100), Point::new(160, 100));
        c.add_wire(Point::new(180, 100), Point::new(240, 100));
        (id, input, output)
    }
    #[test]
    fn nested_instances_are_isolated_and_four_state_signals_cross_boundaries() {
        let mut p = Project::default();
        let (inv, child_input, child_output) = inverter(&mut p, "inv", 4);
        let wrapper = p.add("wrapper".into()).unwrap();
        let inner = p.instance(inv, Point::new(200, 100)).unwrap();
        let inner_id = p.circuit_mut(wrapper).unwrap().circuit.add_component(inner);
        let c = &mut p.circuit_mut(wrapper).unwrap().circuit;
        let mut input = Component::new(Kind::Pin, Point::new(100, 100));
        input.bits = 4;
        c.add_component(input);
        let mut output = Component::new(Kind::Pin, Point::new(250, 100));
        output.bits = 4;
        output.output = true;
        output.facing = Facing::West;
        c.add_component(output);
        c.add_wire(Point::new(100, 100), Point::new(170, 100));
        c.add_wire(Point::new(200, 100), Point::new(250, 100));
        let mut ids = vec![];
        for (i, value) in [10, 15].into_iter().enumerate() {
            let y = 100 + i as i32 * 100;
            let instance = p.instance(wrapper, Point::new(200, y)).unwrap();
            let c = &mut p.main_circuit_mut().circuit;
            let instance = c.add_component(instance);
            let mut input = Component::new(Kind::Pin, Point::new(100, y));
            input.bits = 4;
            let input = c.add_component(input);
            c.add_wire(Point::new(100, y), Point::new(170, y));
            ids.push((instance, input, value));
        }
        p.refresh_interfaces(false);
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, p.main);
        for (_, input, value) in &ids {
            sim.set_input(*input, *value);
        }
        sim.advance(64);
        for (instance, _, value) in &ids {
            let leaf = sim.view(&[*instance, inner_id]).unwrap();
            assert_eq!(
                leaf.port_value(child_input, 0).unwrap().number(),
                Some(*value)
            );
            assert_eq!(
                leaf.port_value(child_output, 0).unwrap().number(),
                Some(15 ^ value)
            );
        }
        // A disconnected parent input must replace the nested Pin's standalone zero drive.
        p.main_circuit_mut().circuit.wires.clear();
        sim.synchronize(&p, p.main);
        sim.advance(64);
        let leaf = sim.view(&[ids[0].0, inner_id]).unwrap();
        assert_eq!(
            leaf.port_value(child_input, 0).unwrap().bit(0),
            Bit::Unknown
        );
        assert_eq!(leaf.port_value(child_output, 0).unwrap().bit(0), Bit::Error);
    }
    #[test]
    fn tunnels_and_multiple_output_ports_are_local_to_each_instance() {
        let mut p = Project::default();
        let sub = p.add("pass".into()).unwrap();
        let c = &mut p.circuit_mut(sub).unwrap().circuit;
        let mut input = Component::new(Kind::Pin, Point::new(100, 100));
        input.bits = 4;
        c.add_component(input);
        for x in [100, 200] {
            let mut t = Component::new(Kind::Tunnel, Point::new(x, 100));
            t.bits = 4;
            t.label = "local".into();
            c.add_component(t);
        }
        for (y, value) in [(100, None), (200, Some(3))] {
            let mut pin = Component::new(Kind::Pin, Point::new(200, y));
            pin.bits = 4;
            pin.output = true;
            pin.facing = Facing::West;
            c.add_component(pin);
            if let Some(value) = value {
                let mut source = Component::new(Kind::Constant, Point::new(200, y));
                source.bits = 4;
                source.constant = value;
                c.add_component(source);
            }
        }
        let mut outputs = vec![];
        for (i, value) in [10, 12].into_iter().enumerate() {
            let y = 100 + i as i32 * 100;
            let instance = p.instance(sub, Point::new(200, y)).unwrap();
            let ports = instance.interface.ports.clone();
            let c = &mut p.main_circuit_mut().circuit;
            let id = c.add_component(instance);
            let mut source = Component::new(Kind::Constant, Point::new(170, y));
            source.bits = 4;
            source.constant = value;
            c.add_component(source);
            outputs.push((id, value, ports));
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, 1);
        sim.advance(64);
        for (id, value, ports) in outputs {
            let outputs: Vec<_> = ports
                .iter()
                .enumerate()
                .filter(|(_, p)| p.output)
                .map(|(i, _)| sim.view(&[]).unwrap().port_value(id, i).unwrap().number())
                .collect();
            assert_eq!(outputs, vec![Some(value), Some(3)]);
        }
        sim.set_enabled(false);
        sim.reset();
        assert!(!sim.enabled());
        while sim.step() {}
        assert_eq!(sim.pending_count(), 0);
    }
    #[test]
    fn deep_or_recursive_projects_fail_without_partial_simulation() {
        let mut p = Project::default();
        let mut parent = 1;
        for i in 0..34 {
            let next = p.add(format!("c{i}")).unwrap();
            let c = p.instance(next, Point::ZERO).unwrap();
            p.circuit_mut(parent).unwrap().circuit.add_component(c);
            parent = next;
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, 1);
        assert!(sim.error().unwrap().contains("32"));
        assert!(sim.view(&[]).is_none());
        let c = p.instance(1, Point::ZERO).unwrap();
        p.circuit_mut(parent).unwrap().circuit.add_component(c);
        sim.synchronize(&p, 1);
        assert!(sim.error().unwrap().contains("recursive"));
    }
    #[test]
    fn standalone_navigation_retains_inputs_and_reset_clears_them() {
        let mut p = Project::default();
        let (child, _, _) = inverter(&mut p, "child", 1);
        let pin = p
            .main_circuit_mut()
            .circuit
            .add_component(Component::new(Kind::Pin, Point::ZERO));
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, 1);
        sim.set_input(pin, 1);
        sim.advance(64);
        sim.synchronize(&p, child);
        sim.advance(64);
        sim.synchronize(&p, 1);
        sim.advance(64);
        assert_eq!(sim.input(pin).unwrap().number(), Some(1));
        sim.synchronize(&p, child);
        sim.reset();
        sim.synchronize(&p, 1);
        sim.advance(64);
        assert_eq!(sim.input(pin).unwrap().number(), Some(0));
    }
    #[test]
    fn custom_package_rotation_works_and_visual_edits_preserve_paused_signals() {
        let mut p = Project::default();
        let (child, input, output) = inverter(&mut p, "pack", 4);
        let mut art = p.circuit(child).unwrap().editable_appearance();
        art.facing = Facing::North;
        p.circuit_mut(child).unwrap().circuit.appearance = Some(art);
        let mut instance = p.instance(child, Point::new(200, 200)).unwrap();
        instance.facing = Facing::West;
        let input_index = instance
            .interface
            .ports
            .iter()
            .position(|port| port.pin == input)
            .unwrap();
        let output_index = instance
            .interface
            .ports
            .iter()
            .position(|port| port.pin == output)
            .unwrap();
        let mut source = Component::new(Kind::Constant, instance.ports()[input_index]);
        source.bits = 4;
        source.constant = 10;
        let id = p.main_circuit_mut().circuit.add_component(instance);
        p.main_circuit_mut().circuit.add_component(source);
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&p, p.main);
        sim.advance(64);
        assert_eq!(
            sim.view(&[])
                .unwrap()
                .port_value(id, output_index)
                .unwrap()
                .number(),
            Some(5)
        );
        sim.set_enabled(false);
        let steps = sim.steps();
        let value = sim.view(&[id]).unwrap().port_value(output, 0);
        p.circuit_mut(child)
            .unwrap()
            .circuit
            .appearance
            .as_mut()
            .unwrap()
            .shapes[0]
            .paint
            .fill = Some([100, 150, 200]);
        p.refresh_interfaces(true);
        assert!(!sim.synchronize(&p, p.main));
        assert!(!sim.enabled());
        assert_eq!(sim.steps(), steps);
        assert_eq!(sim.view(&[id]).unwrap().port_value(output, 0), value);
    }
}
