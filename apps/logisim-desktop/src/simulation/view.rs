//! Read-only adaptation from simulation state to UI values and localized diagnostics.
use super::values::{input_value, probe_text, signal};
use logisim_core::{Circuit, Kind};
use logisim_sim::{Bit, Problem, Simulator};
use logisim_ui::{SimulationIssue, SimulationView, i18n::tr};

fn present(circuit: &Circuit, sim: &Simulator) -> SimulationView {
    let mut view = SimulationView {
        enabled: sim.enabled(),
        pending: sim.pending_count(),
        steps: sim.steps(),
        limited: sim.limited(),
        ..Default::default()
    };
    for wire in &circuit.wires {
        if let Some(value) = sim.wire_value(wire.id) {
            view.wires
                .insert(wire.id, signal(value, sim.wire_mismatch(wire.id)));
        }
    }
    for c in &circuit.components {
        if let Some(graphics) = sim.plugin_graphics(c.id) {
            view.plugin_graphics.insert(c.id, graphics.to_vec());
        }
        for index in 0..c.ports().len() {
            if let Some(value) = sim.port_value(c.id, index) {
                let mut presented = signal(value, sim.port_mismatch(c.id, index));
                if c.kind == Kind::Probe {
                    presented.text = probe_text(value, c.radix);
                }
                view.ports.insert((c.id, index), presented);
            }
        }
        if c.kind == Kind::Clock {
            view.clocks.insert(c.id);
        }
        if let Some(words) = sim.register_file(c.id) {
            view.register_files
                .insert(c.id, words.iter().map(|v| signal(*v, false)).collect());
        }
        if let Some(contents) = sim.memory_contents(c.id) {
            view.memories.insert(c.id, contents.clone());
        }
        if let Some(value) = sim.stored_value(c.id) {
            view.storage.insert(c.id, signal(value, false));
        }
        if let Some(rgb) = sim.rgb_color(c.id) {
            view.rgb.insert(c.id, rgb);
        }
        if let Some(cells) = sim.matrix_cells(c.id) {
            view.matrices.insert(
                c.id,
                cells
                    .into_iter()
                    .map(|bit| match bit {
                        Bit::One => Some(true),
                        Bit::Zero => Some(false),
                        _ => None,
                    })
                    .collect(),
            );
        }
        if let Some(buffer) = sim.keyboard_buffer(c.id) {
            view.keyboards.insert(c.id, buffer);
        }
        if let Some(screen) = sim.tty(c.id) {
            view.ttys.insert(
                c.id,
                logisim_ui::TtyView {
                    lines: screen.lines.clone(),
                    row: screen.row,
                    column: screen.column,
                },
            );
        }
        if let Some(offset) = sim.joystick(c.id) {
            view.joysticks.insert(c.id, offset);
        }
        if let Some(lights) = sim.indicator(c.id) {
            view.indicators.insert(c.id, lights);
        }
        if c.kind == Kind::Button {
            view.buttons
                .insert(c.id, sim.input(c.id).is_some_and(|v| v.number() == Some(1)));
        }
        if c.kind != Kind::Button
            && let Some(value) = sim.input(c.id)
        {
            view.inputs.insert(c.id, input_value(value));
        }
    }
    view.has_clocks = sim.has_clocks();
    append_issues(&mut view, circuit, sim.diagnostics());
    for (id, error) in sim.plugin_errors() {
        view.plugin_errors.insert(id, error.into());
    }
    view
}
fn append_issues(
    view: &mut SimulationView,
    circuit: &Circuit,
    diagnostics: Vec<logisim_sim::Diagnostic>,
) {
    for diagnostic in diagnostics {
        let message = match diagnostic.problem {
            Problem::WidthMismatch => "Incompatible bit widths",
            Problem::ErrorSignal => "Error signal: conflicting drivers / undefined input",
            Problem::PropagationLimit => "Propagation limit: check feedback, then Reset or Run",
        };
        let names: Vec<_> = diagnostic
            .components
            .iter()
            .take(3)
            .filter_map(|id| circuit.component(*id))
            .map(|c| {
                if c.label.is_empty() {
                    format!("{} #{}", tr(c.kind.name()), c.id)
                } else {
                    c.label.clone()
                }
            })
            .collect();
        view.issues.push(SimulationIssue {
            message: format!("{}\n{}", tr(message), names.join(", ")),
            targets: diagnostic
                .components
                .into_iter()
                .chain(diagnostic.wires)
                .collect(),
        });
    }
}
pub fn present_project(
    circuit: &Circuit,
    sim: &logisim_sim::ProjectSimulator,
    path: &[logisim_core::Id],
) -> SimulationView {
    let mut view = sim
        .view(path)
        .map(|s| present(circuit, s))
        .unwrap_or_default();
    view.enabled = sim.enabled();
    view.pending = sim.pending_count();
    view.steps = sim.steps();
    view.limited = sim.limited();
    view.has_clocks = sim.has_clocks();
    view.ticks = sim.ticks();
    if !path.is_empty() {
        view.inputs.retain(|id, _| {
            circuit
                .component(*id)
                .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch))
        });
    }
    view.issues.clear();
    let plugin_failures = sim.plugin_failures();
    view.plugin_error_count = plugin_failures.len();
    for (instance, id, error) in plugin_failures {
        if instance.starts_with(path) {
            let target = instance.get(path.len()).copied().unwrap_or(id);
            view.issues.push(SimulationIssue {
                message: format!("{} #{}: {error}", tr("Plugin"), id),
                targets: std::collections::BTreeSet::from([target]),
            });
        }
    }
    append_issues(&mut view, circuit, sim.diagnostics(path));
    if let Some(error) = sim.error() {
        view.issues.push(SimulationIssue {
            message: tr(error).into(),
            targets: Default::default(),
        });
    }
    view
}
