//! Explicit stimuli and propagation boundaries for a single headless session.
use super::Runtime;
use crate::{Error, Result};
use logisim_core::{Bit, Id, Kind, Project, Value as Signal};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) enum Input {
    Known(u32),
    Unknown,
}

pub(crate) enum Action {
    SetInput(Id, Input),
    Settle,
    Step,
    Tick,
    Reset,
    SetEnabled(bool),
}

impl Runtime {
    fn require_ready(&self) -> Result<()> {
        if let Some(reason) = &self.failure {
            return Err(Error::new("invalid_state", reason));
        }
        Ok(())
    }

    fn settle_pending(&mut self) -> bool {
        if self.sim.pending_count() == 0 {
            return false;
        }
        self.propagate();
        true
    }

    fn set_input(&mut self, project: &Project, id: Id, input: Input) -> Result<bool> {
        let component = project
            .main_circuit()
            .circuit
            .component(id)
            .filter(|c| matches!(c.kind, Kind::Pin | Kind::Switch | Kind::DipSwitch))
            .ok_or_else(|| {
                Error::new(
                    "invalid_input",
                    "Expected a root input Pin, Switch or DipSwitch.",
                )
            })?;
        let current = self
            .sim
            .input(id)
            .ok_or_else(|| Error::new("invalid_input", "This component is not an input."))?;
        let width = current.width();
        let target = match input {
            Input::Known(n) if width == 32 || n < (1u32 << width) => Signal::known(width, n),
            Input::Known(_) => {
                return Err(Error::new(
                    "invalid_input",
                    format!("Value exceeds {width}-bit input."),
                ));
            }
            Input::Unknown if component.kind == Kind::Pin && component.pin.three_state => {
                Signal::filled(width, Bit::Unknown)
            }
            Input::Unknown => {
                return Err(Error::new(
                    "invalid_input",
                    "Unknown requires a three-state input Pin.",
                ));
            }
        };
        if target != current {
            match input {
                Input::Known(n) => self.sim.set_input(id, n),
                Input::Unknown => self.sim.set_input_unknown_at(&[], id),
            }
        }
        let propagated = self.sim.enabled() && self.settle_pending();
        Ok(target != current || propagated)
    }

    pub(crate) fn control(
        &mut self,
        project: &Project,
        session: &str,
        action: Action,
    ) -> Result<Value> {
        // Reset is an explicit recovery path. A propagation limit can also be retried by resuming.
        match action {
            Action::Reset | Action::SetEnabled(false) => {}
            Action::SetEnabled(true) if self.sim.limited() => {}
            _ => self.require_ready()?,
        }

        let mut reason = None;
        let mut before_reset = None;
        let (name, changed) = match action {
            Action::SetInput(id, input) => ("set_input", self.set_input(project, id, input)?),
            Action::Settle => ("settle", self.settle_pending()),
            Action::Step => {
                let advanced = self.sim.step();
                self.check_execution();
                ("step", advanced)
            }
            Action::Tick => {
                if self.sim.pending_count() != 0 {
                    let mut error =
                        Error::new("not_settled", "Settle pending propagation before ticking.");
                    error.details["pending_count"] = self.sim.pending_count().into();
                    return Err(error);
                }
                if self.sim.has_clock_source() {
                    self.sim.tick();
                    self.settle_pending();
                    ("tick", true)
                } else {
                    reason = Some("no_clocks");
                    ("tick", false)
                }
            }
            Action::Reset => {
                before_reset = Some(self.state(project, session));
                self.sim.reset();
                self.step_origin = 0;
                self.initialize();
                // Initialization itself propagates. Start the user-visible counter at the settled reset boundary.
                if self.failure.is_none() {
                    self.step_origin = self.sim.steps();
                }
                ("reset", true)
            }
            Action::SetEnabled(enabled) => {
                let toggled = enabled != self.sim.enabled();
                if toggled {
                    self.sim.set_enabled(enabled);
                    if enabled {
                        self.failure = None;
                    }
                }
                let propagated = enabled && self.settle_pending();
                ("set_enabled", toggled || propagated)
            }
        };
        let state = self.state(project, session);
        if !matches!(action, Action::SetEnabled(false))
            && let Err(mut error) = self.require_ready()
        {
            error.details["state"] = state;
            return Err(error);
        }
        let changed = before_reset.map_or(changed, |old| old != state);
        let mut result =
            json!({"session_id":session, "action":name, "changed":changed, "state":state});
        if let Some(reason) = reason {
            result["reason"] = reason.into();
        }
        Ok(result)
    }
}
