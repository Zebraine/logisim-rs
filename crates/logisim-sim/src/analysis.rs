//! Read-only Boolean analysis of an entire stateless circuit definition.
//! Uses an isolated runtime and the same topology/operations as normal simulation.
//! Unknown/error signals are never interpreted as synthesis don't-cares.
#[cfg(test)]
mod tests;

use crate::{
    ProjectSimulator,
    combinational::{BoundaryReason, Classification, classify},
    stable,
};
use logisim_core::{
    Component, Id, Kind, Project,
    generation::{Analysis, Cell, Error},
};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, Error>;
fn error(message: &'static str, detail: impl ToString) -> Error {
    Error {
        message,
        detail: detail.to_string(),
    }
}
fn location(name: &str, c: &Component) -> String {
    format!(
        "{name} / {} #{} ({}, {})",
        c.label, c.id, c.position.x, c.position.y
    )
}
fn check_cancel(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err(error("Circuit analysis cancelled.", ""))
    } else {
        Ok(())
    }
}

pub fn analyze(project: &Project, root: Id) -> Result<Analysis> {
    analyze_cancellable(project, root, || false)
}

/// The callback is checked during validation and between bounded evaluation batches.
/// The document, its labels and any live simulator are never modified.
pub fn analyze_cancellable(
    project: &Project,
    root: Id,
    cancelled: impl Fn() -> bool,
) -> Result<Analysis> {
    check_cancel(&cancelled)?;
    project
        .validate()
        .map_err(|e| error("Invalid circuit project:", e))?;
    let circuit = &project
        .circuit(root)
        .ok_or_else(|| error("Missing circuit to analyze.", root))?
        .circuit;
    let pins = logisim_core::generation::analysis_pins(circuit);
    if !pins.iter().any(|p| p.output) {
        return Err(error("Analysis requires at least one output Pin.", ""));
    }
    let mut analysis = Analysis::default();
    let mut used: BTreeSet<_> = pins
        .iter()
        .filter(|p| !p.label.is_empty())
        .map(|p| p.label.clone())
        .collect();
    for p in &pins {
        if p.bits != 1 {
            return Err(error(
                "Analysis requires single-bit input and output Pins.",
                &p.label,
            ));
        }
        let name = if p.label.is_empty() {
            fresh_name(p.output, &mut used)
        } else {
            p.label.clone()
        };
        let result = if p.output {
            analysis.add_output(name)
        } else {
            analysis.add_input(name)
        };
        result.map_err(|e| {
            error(
                e.message,
                format!("{} #{} ({}, {})", p.label, p.id, p.position.x, p.position.y),
            )
        })?;
    }
    validate_stateless(project, root, &cancelled)?;
    check_cancel(&cancelled)?;
    let mut sim = ProjectSimulator::default();
    sim.synchronize(project, root);
    sim.prepare_compiled().map_err(evaluation_error)?;
    let inputs: Vec<_> = pins.iter().filter(|p| !p.output).map(|p| p.id).collect();
    let outputs: Vec<_> = pins.iter().filter(|p| p.output).map(|p| p.id).collect();
    for row in 0..analysis.rows() {
        check_cancel(&cancelled)?;
        for (i, id) in inputs.iter().enumerate() {
            sim.set_input(*id, u32::from(analysis.input_value(row, i).unwrap()));
        }
        loop {
            check_cancel(&cancelled)?;
            sim.step_compiled().map_err(evaluation_error)?;
            if sim.pending_count() == 0 {
                break;
            }
        }
        if !sim.diagnostics(&[]).is_empty() {
            return Err(error(
                "Circuit analysis found an electrical error at input row:",
                row,
            ));
        }
        for (output, id) in outputs.iter().enumerate() {
            let value = sim
                .view(&[])
                .and_then(|s| s.port_value(*id, 0))
                .and_then(|v| v.number());
            let Some(value @ (0 | 1)) = value else {
                return Err(error(
                    "An output is unknown or erroneous; analysis requires 0 or 1 for every input row:",
                    format!("{} / {row}", analysis.outputs()[output].name()),
                ));
            };
            analysis.set_cell(output, row, Cell::from(value == 1))?;
        }
    }
    check_cancel(&cancelled)?;
    Ok(analysis)
}

fn fresh_name(output: bool, used: &mut BTreeSet<String>) -> String {
    let letters = if output { "xyzuvwst" } else { "abcdefgh" };
    for letter in letters.chars() {
        let name = letter.to_string();
        if used.insert(name.clone()) {
            return name;
        }
    }
    for i in 1.. {
        let name = format!("{}{i}", if output { "out" } else { "in" });
        if used.insert(name.clone()) {
            return name;
        }
    }
    unreachable!()
}

fn validate_stateless(project: &Project, root: Id, cancelled: &impl Fn() -> bool) -> Result<()> {
    let mut stack = vec![(root, 0)];
    let (mut instances, mut items) = (0, 0);
    while let Some((id, depth)) = stack.pop() {
        check_cancel(cancelled)?;
        instances += 1;
        if depth > 32 || instances > 1024 {
            return Err(error("Circuit analysis exceeds the hierarchy limit.", ""));
        }
        let definition = project
            .circuit(id)
            .ok_or_else(|| error("Missing circuit to analyze.", id))?;
        let c = &definition.circuit;
        let count = c.components.len() + c.wires.len();
        items += count;
        if count > logisim_core::MAX_CIRCUIT_ITEMS || items > logisim_core::MAX_PROJECT_ITEMS {
            return Err(error(
                "Circuit analysis exceeds the item limit.",
                &definition.name,
            ));
        }
        let mut ids = BTreeSet::new();
        if !c
            .components
            .iter()
            .map(|c| c.id)
            .chain(c.wires.iter().map(|w| w.id))
            .all(|id| ids.insert(id))
        {
            return Err(error("Invalid circuit project:", &definition.name));
        }
        for component in &c.components {
            check_cancel(cancelled)?;
            if let Kind::Subcircuit(child) = component.kind {
                let child_definition = project
                    .circuit(child)
                    .ok_or_else(|| error("Missing circuit to analyze.", child))?;
                let interface = child_definition.interface();
                if component.interface != interface
                    || interface.ports.len()
                        != child_definition
                            .circuit
                            .components
                            .iter()
                            .filter(|c| c.kind == Kind::Pin)
                            .count()
                {
                    return Err(error(
                        "Circuit analysis requires complete, valid subcircuit interfaces:",
                        location(&definition.name, component),
                    ));
                }
                stack.push((child, depth + 1));
                continue;
            }
            if let Classification::Boundary(reason) = classify(component) {
                let message = match reason {
                    BoundaryReason::ExternalInput => continue,
                    BoundaryReason::State => {
                        "Circuit analysis does not support timing or stateful components:"
                    }
                    BoundaryReason::Interaction => {
                        "Circuit analysis does not support interactive inputs; use input Pins:"
                    }
                    BoundaryReason::Plugin => {
                        "Circuit analysis cannot verify that a plugin is stateless:"
                    }
                    _ => "Circuit analysis does not support this component:",
                };
                return Err(error(message, location(&definition.name, component)));
            }
        }
    }
    Ok(())
}
fn evaluation_error(e: stable::Error) -> Error {
    let message = match e.reason {
        stable::Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        } => "Circuit analysis does not support feedback connections.",
        stable::Failure::Topology {
            reason: BoundaryReason::WidthMismatch,
            ..
        } => "Circuit analysis found incompatible signal widths.",
        stable::Failure::PropagationLimit => "Circuit analysis exceeded the settling limit.",
        _ => "Circuit analysis could not evaluate this circuit.",
    };
    error(message, e)
}
