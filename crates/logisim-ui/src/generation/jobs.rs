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
enum Destination {
    New(String),
    Replace(RebuildTarget),
}
#[derive(Clone)]
enum Operation {
    Mode {
        expressions: bool,
    },
    Minimize {
        output: usize,
        form: Form,
        draft: Option<String>,
        editing: bool,
    },
    Build {
        destination: Destination,
        options: GenerationOptions,
    },
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
        expression: Expression,
        editing: bool,
    },
    Built {
        destination: Destination,
        circuit: Box<Circuit>,
        expressions: Analysis,
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
            Operation::Minimize {
                output,
                form,
                draft,
                editing,
            } => {
                // Simplify the current edit in isolation. Dense rows exist only in
                // this bounded background job; the session mode stays unchanged.
                let edited = draft
                    .as_ref()
                    .map(|draft| {
                        let mut analysis = Analysis::expressions();
                        for name in self.analysis.inputs() {
                            analysis.add_input(name.clone())?;
                        }
                        analysis.add_output(self.analysis.outputs()[*output].name().to_owned())?;
                        analysis.apply_expression(0, draft)?;
                        analysis.truth_table(&cancelled)
                    })
                    .transpose()?;
                let (analysis, output) = edited
                    .as_ref()
                    .map_or((&self.analysis, *output), |a| (a, 0));
                let expression = cache.minimized(analysis, output, *form, &cancelled)?;
                Ok(Outcome::Minimized {
                    expression,
                    editing: *editing,
                })
            }
            Operation::Build {
                destination,
                options,
            } => {
                let (circuit, expressions) =
                    cache.generate_with_expressions(&self.analysis, *options, &cancelled)?;
                Ok(Outcome::Built {
                    destination: destination.clone(),
                    circuit: Box::new(circuit),
                    expressions,
                })
            }
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
    fn receive_expression(&mut self, expression: Expression, editing: bool) {
        self.draft = expression.text(self.analysis.inputs());
        if !editing {
            self.applied.clone_from(&self.draft);
        }
        self.error = None;
    }
    /// Populate the expression view when a table has no supplied expression.
    pub(super) fn request_minimize(&mut self) {
        self.minimize(false);
    }
    pub(super) fn request_simplify(&mut self) {
        self.minimize(true);
    }
    fn minimize(&mut self, editing: bool) {
        if self.analysis.outputs().is_empty() {
            return;
        }
        let form = if editing {
            self.form
        } else {
            Form::SumOfProducts
        };
        let draft = (editing && (self.pending() || self.analysis.is_expression_mode()))
            .then(|| self.draft.clone());
        if draft.is_none()
            && let Some(result) = self.cache.cached(&self.analysis, self.output, form)
        {
            self.cancel_computation();
            match result {
                Ok(expression) => self.receive_expression(expression, editing),
                Err(error) => self.error = Some(error),
            }
            return;
        }
        self.request(Operation::Minimize {
            output: self.output,
            form,
            draft,
            editing,
        });
    }
    pub(super) fn request_mode(&mut self, expressions: bool) {
        self.request(Operation::Mode { expressions });
    }
    pub(super) fn request_build(&mut self) {
        self.request(Operation::Build {
            destination: if let Some(target) = self.target.as_ref().filter(|_| self.replace_target)
            {
                Destination::Replace(target.clone())
            } else {
                Destination::New(self.name.trim().to_owned())
            },
            options: GenerationOptions {
                basis: if self.nand_only {
                    GateBasis::Nand
                } else {
                    GateBasis::Standard
                },
                two_input_only: self.two_input_only,
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
                g.tab = if g.analysis.is_expression_mode() || !self.truth_table_generation {
                    Tab::Expression
                } else {
                    Tab::Table
                };
                g.refresh();
            }
            Ok(Outcome::Minimized {
                expression,
                editing,
            }) => {
                g.receive_expression(expression, editing);
            }
            Ok(Outcome::Built {
                destination,
                circuit,
                expressions,
            }) => {
                let result = match destination {
                    Destination::New(name) => self.editor.install_generated(name, *circuit),
                    Destination::Replace(target) => self.editor.install_rebuilt(target, *circuit),
                };
                match result {
                    Ok(()) => {
                        self.generated_expressions.remember(
                            self.editor.project.circuit(self.editor.active).unwrap(),
                            expressions,
                        );
                        // Host replies arrive before show() captures the current
                        // view; explicitly fit the newly installed geometry.
                        self.appearance.reset_context();
                        self.canvas.reset();
                        self.fit_pending = true;
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
