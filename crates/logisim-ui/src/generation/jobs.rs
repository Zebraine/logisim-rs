//! Data-only requests; the desktop host owns threads and cancellation flags.
use super::*;
use logisim_core::generation::Expression;

#[derive(Clone)]
pub struct Request {
    pub serial: u64,
    analysis: Analysis,
    operation: Operation,
}
#[derive(Clone)]
enum Operation {
    Minimize { output: usize, form: Form },
    Build { name: String, basis: GateBasis },
}
pub enum Reply {
    Minimized {
        applied: Expression,
        minimized: Expression,
    },
    Built {
        name: String,
        circuit: Circuit,
    },
}
impl Request {
    pub fn execute(&self, cancelled: impl Fn() -> bool) -> Result<Reply, Error> {
        match &self.operation {
            Operation::Minimize { output, form } => {
                let minimized = self
                    .analysis
                    .minimized_cancellable(*output, *form, &cancelled)?;
                let applied = match self.analysis.supplied_expression(*output) {
                    Some(e) => e.clone(),
                    None if *form == Form::SumOfProducts => minimized.clone(),
                    None => self.analysis.minimized_cancellable(
                        *output,
                        Form::SumOfProducts,
                        &cancelled,
                    )?,
                };
                Ok(Reply::Minimized { applied, minimized })
            }
            Operation::Build { name, basis } => Ok(Reply::Built {
                name: name.clone(),
                circuit: self.analysis.generate_cancellable(*basis, cancelled)?,
            }),
        }
    }
}
impl Generator {
    pub(crate) fn cancel_computation(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.computing = false;
        self.building = false;
        self.requested = None;
    }
    fn request(&mut self, operation: Operation) {
        self.cancel_computation();
        self.building = matches!(operation, Operation::Build { .. });
        self.computing = true;
        self.error = None;
        self.requested = Some(Request {
            serial: self.serial,
            analysis: self.analysis.clone(),
            operation,
        });
    }
    pub(super) fn request_minimize(&mut self) {
        if !self.analysis.outputs().is_empty() {
            self.request(Operation::Minimize {
                output: self.output,
                form: self.form,
            });
        }
    }
    pub(super) fn request_build(&mut self) {
        self.request(Operation::Build {
            name: self.name.trim().to_owned(),
            basis: if self.nand_only {
                GateBasis::Nand
            } else {
                GateBasis::Standard
            },
        });
    }
}
impl Workspace {
    pub fn take_generation_request(&mut self) -> Option<Request> {
        self.generator
            .open
            .then(|| self.generator.requested.take())
            .flatten()
    }
    pub fn generation_pending(&self, serial: u64) -> bool {
        self.generator.open && self.generator.computing && self.generator.serial == serial
    }
    pub fn receive_generation(&mut self, serial: u64, result: Result<Reply, Error>) {
        if !self.generation_pending(serial) {
            return;
        }
        let g = &mut self.generator;
        g.computing = false;
        g.building = false;
        g.requested = None;
        match result {
            Ok(Reply::Minimized { applied, minimized }) => {
                g.applied = applied.text(g.analysis.inputs());
                g.draft.clone_from(&g.applied);
                g.minimized = minimized.text(g.analysis.inputs());
                g.error = None;
            }
            Ok(Reply::Built { name, circuit }) => {
                match self.editor.install_generated(name, circuit) {
                    Ok(()) => {
                        g.cancel_computation();
                        g.open = false;
                    }
                    Err(e) => g.error = Some(e),
                }
            }
            Err(e) => g.error = Some(e),
        }
    }
}
