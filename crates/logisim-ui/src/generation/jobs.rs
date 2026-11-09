//! Data-only requests; the desktop host owns threads and cancellation flags.
use super::*;
use logisim_core::generation::Expression;

#[derive(Clone)]
pub struct Request {
    pub serial: u64,
    analysis: Analysis,
    operation: Operation,
    cache: logisim_core::generation::Minimizer,
}
#[derive(Clone)]
enum Operation {
    Mode { expressions: bool },
    Minimize { output: usize, form: Form },
    Build { name: String, basis: GateBasis },
}
pub struct Reply {
    cache: logisim_core::generation::Minimizer,
    result: Result<Outcome, Error>,
}
enum Outcome {
    Converted {
        analysis: Analysis,
    },
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
        let mut cache = self.cache.clone();
        let result = (|| match &self.operation {
            Operation::Mode { expressions } => Ok(Outcome::Converted {
                analysis: if *expressions {
                    cache.expressions(&self.analysis, &cancelled)?
                } else {
                    self.analysis.truth_table(&cancelled)?
                },
            }),
            Operation::Minimize { output, form } => {
                let minimized = cache.minimized(&self.analysis, *output, *form, &cancelled)?;
                let applied = match self.analysis.supplied_expression(*output) {
                    Some(e) => e.clone(),
                    None if *form == Form::SumOfProducts => minimized.clone(),
                    None => {
                        cache.minimized(&self.analysis, *output, Form::SumOfProducts, &cancelled)?
                    }
                };
                Ok(Outcome::Minimized { applied, minimized })
            }
            Operation::Build { name, basis } => Ok(Outcome::Built {
                name: name.clone(),
                circuit: cache.generate(&self.analysis, *basis, &cancelled)?,
            }),
        })();
        Ok(Reply { cache, result })
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
        self.building = matches!(operation, Operation::Build { .. } | Operation::Mode { .. });
        self.computing = true;
        self.error = None;
        self.requested = Some(Request {
            serial: self.serial,
            analysis: self.analysis.clone(),
            operation,
            cache: self.cache.clone(),
        });
    }
    pub(super) fn request_minimize(&mut self) {
        if self.analysis.is_expression_mode() || self.analysis.outputs().is_empty() {
            return;
        }
        if let Some(result) = self.cache.cached(&self.analysis, self.output, self.form) {
            match result {
                Ok(minimized) => {
                    let applied = self
                        .analysis
                        .supplied_expression(self.output)
                        .cloned()
                        .map(Ok)
                        .or_else(|| {
                            self.cache
                                .cached(&self.analysis, self.output, Form::SumOfProducts)
                        });
                    if let Some(Ok(applied)) = applied {
                        self.applied = applied.text(self.analysis.inputs());
                        self.draft.clone_from(&self.applied);
                        self.minimized = minimized.text(self.analysis.inputs());
                        return;
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        self.request(Operation::Minimize {
            output: self.output,
            form: self.form,
        });
    }
    pub(super) fn request_mode(&mut self, expressions: bool) {
        self.request(Operation::Mode { expressions });
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
        let result = result.and_then(|reply| {
            g.cache = reply.cache;
            reply.result
        });
        match result {
            Ok(Outcome::Converted { analysis }) => {
                g.analysis = analysis;
                g.tab = if g.analysis.is_expression_mode() {
                    Tab::Expression
                } else {
                    Tab::Table
                };
                g.refresh();
            }
            Ok(Outcome::Minimized { applied, minimized }) => {
                g.applied = applied.text(g.analysis.inputs());
                g.draft.clone_from(&g.applied);
                g.minimized = minimized.text(g.analysis.inputs());
                g.error = None;
            }
            Ok(Outcome::Built { name, circuit }) => {
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
