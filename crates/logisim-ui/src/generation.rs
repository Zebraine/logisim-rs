//! The analyzer is a session draft. Only Build commits an editable circuit.
mod jobs;
pub use jobs::{Reply, Request};

use crate::{Workspace, editor::Editor, i18n::tr, tf};
use egui::{Color32, Ui};
use logisim_core::{
    Circuit,
    generation::{
        Analysis, Error, Form, GateBasis, GenerationOptions, MAX_EXPRESSION_BYTES, RebuildTarget,
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Inputs,
    Outputs,
    Table,
    Expression,
}

#[derive(Default)]
struct VariableEditor {
    selected: Option<usize>,
    name: String,
}

pub(crate) struct Generator {
    pub open: bool,
    pub(super) serial: u64,
    computing: bool,
    building: bool,
    requested: Option<Request>,
    cache: logisim_core::generation::Minimizer,
    analysis: Analysis,
    tab: Tab,
    inputs: VariableEditor,
    outputs: VariableEditor,
    output: usize,
    draft: String,
    applied: String,
    form: Form,
    nand_only: bool,
    two_input_only: bool,
    name: String,
    pub(super) target: Option<RebuildTarget>,
    pub(super) replace_target: bool,
    error: Option<Error>,
}
impl Default for Generator {
    fn default() -> Self {
        let mut analysis = Analysis::expressions();
        for name in ["A", "B"] {
            analysis.add_input(name.into()).unwrap();
        }
        analysis.add_output("Y".into()).unwrap();
        Self {
            open: false,
            serial: 0,
            computing: false,
            building: false,
            requested: None,
            cache: Default::default(),
            analysis,
            tab: Tab::Inputs,
            inputs: Default::default(),
            outputs: Default::default(),
            output: 0,
            draft: "0".into(),
            applied: "0".into(),
            form: Form::default(),
            nand_only: false,
            two_input_only: false,
            name: "generated".into(),
            target: None,
            replace_target: false,
            error: None,
        }
    }
}
impl Generator {
    /// Hide the table without converting or discarding its contents or a pending draft.
    pub(super) fn hide_truth_table(&mut self) {
        if self.tab == Tab::Table {
            self.tab = Tab::Expression;
        }
    }
    pub(crate) fn load_analysis(&mut self, analysis: Analysis) {
        self.cancel_computation();
        *self = Self {
            serial: self.serial,
            analysis,
            tab: Tab::Table,
            name: "analyzed".into(),
            ..Self::default()
        };
        self.refresh();
    }
    fn pending(&self) -> bool {
        self.draft != self.applied
    }
    fn refresh(&mut self) {
        self.cancel_computation();
        self.output = self
            .output
            .min(self.analysis.outputs().len().saturating_sub(1));
        self.applied = self
            .analysis
            .supplied_expression(self.output)
            .map(|e| e.text(self.analysis.inputs()))
            .unwrap_or_default();
        self.draft.clone_from(&self.applied);
        self.error = None;
        if self.applied.is_empty() {
            self.request_minimize();
        }
    }
    fn result(&mut self, result: Result<(), Error>) {
        match result {
            Ok(()) => self.refresh(),
            Err(error) => self.error = Some(error),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, truth_table_generation: bool) {
        if !self.open {
            return;
        }
        if !truth_table_generation {
            self.hide_truth_table();
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("circuit-generator")).show(ctx, |ui| {
            let available = ctx.content_rect().size();
            ui.set_width((available.x - 80.0).clamp(280.0, 760.0));
            ui.heading(tr("Combinational Circuit Generator"));
            if truth_table_generation {
                ui.weak(tr("Truth tables: 12 inputs · Expressions: 32 inputs · Outputs: 32"));
            } else {
                let inputs = if self.analysis.is_expression_mode() {
                    logisim_core::generation::MAX_INPUTS
                } else {
                    logisim_core::generation::MAX_TABLE_INPUTS
                };
                ui.weak(tf!("Inputs: {} · Outputs: {}", inputs, logisim_core::generation::MAX_OUTPUTS));
            }
            let pending = self.pending();
            let mut expression_mode = self.analysis.is_expression_mode();
            if truth_table_generation && ui.add_enabled(!pending && !self.computing, egui::Checkbox::new(&mut expression_mode, tr("Expression mode"))).on_hover_text(tr("Switching to expressions resolves all x entries to 0 or 1.")).changed() {
                self.request_mode(expression_mode);
            }
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (Tab::Inputs, "Input variables"),
                    (Tab::Outputs, "Output variables"),
                    (Tab::Table, "Truth Table"),
                    (Tab::Expression, "Expression"),
                ] {
                    if tab == Tab::Table && !truth_table_generation {
                        continue;
                    }
                    if ui
                        .add_enabled(
                            !pending || tab == Tab::Expression,
                            egui::Button::selectable(self.tab == tab, tr(label)),
                        )
                        .clicked()
                    {
                        self.tab = tab;
                        self.error = None;
                    }
                }
            });
            ui.separator();
            let height = (available.y - 280.0).clamp(120.0, 380.0);
            ui.add_enabled_ui(!self.building, |ui| {
                if self.tab == Tab::Table {
                    self.table(ui, height);
                } else {
                    egui::ScrollArea::vertical().id_salt("generator-body")
                        .max_height(height).min_scrolled_height(height).show(ui, |ui| {
                            ui.set_min_height(height);
                            match self.tab {
                                Tab::Inputs => self.variables(ui, true, height),
                                Tab::Outputs => self.variables(ui, false, height),
                                Tab::Expression => self.expression(ui),
                                Tab::Table => unreachable!(),
                            }
                        });
                }
            });
            ui.separator();
            ui.add_enabled_ui(!self.building, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.two_input_only, tr("Use two-input gates only"));
                    ui.checkbox(&mut self.nand_only, tr("Use NAND gates only"))
                        .on_hover_text(tr("Convert all logic gates, including XOR, to NAND gates. Pins and constants are retained."));
                });
            });
            if pending {
                ui.weak(tr("Apply or revert the expression before continuing."));
            }
            if let Some(error) = &self.error {
                ui.colored_label(
                    Color32::from_rgb(185, 28, 28),
                    format!("{} {}", tr(error.message), error.detail),
                );
            }
            ui.horizontal_wrapped(|ui| {
                if let Some(target) = &self.target {
                    ui.add_enabled(!self.building, egui::Checkbox::new(
                        &mut self.replace_target, tf!("Replace circuit: {}", target.name()),
                    ));
                }
                if !self.replace_target {
                    ui.label(tr("New circuit name"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .id_salt("generated-name")
                            .char_limit(1024)
                            .desired_width(200.0),
                    );
                }
                if ui
                    .add_enabled(
                        !self.pending() && !self.building && !self.analysis.outputs().is_empty(),
                        egui::Button::new(tr("Build Circuit")),
                    )
                    .clicked()
                {
                    self.request_build();
                }
                if self.computing {
                    ui.spinner();
                    ui.label(tr("Computing…"));
                    if ui.button(tr("Cancel")).clicked() { self.cancel_computation(); }
                }
                if ui.button(tr("Close")).clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.cancel_computation();
            self.open = false;
        }
    }
    fn variables(&mut self, ui: &mut Ui, input: bool, height: f32) {
        let limit = if !input {
            logisim_core::generation::MAX_OUTPUTS
        } else if self.analysis.is_expression_mode() {
            logisim_core::generation::MAX_INPUTS
        } else {
            logisim_core::generation::MAX_TABLE_INPUTS
        };
        let names: Vec<_> = if input {
            self.analysis.inputs().to_vec()
        } else {
            self.analysis
                .outputs()
                .iter()
                .map(|o| o.name().to_owned())
                .collect()
        };
        let fields = if input {
            &mut self.inputs
        } else {
            &mut self.outputs
        };
        let mut action = None;
        ui.horizontal_top(|ui| {
            egui::ScrollArea::vertical()
                .id_salt(("generator-variable-list", input))
                .max_width(220.0)
                .max_height((height - 160.0).clamp(60.0, 200.0))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(200.0);
                        for (index, name) in names.iter().enumerate() {
                            if ui
                                .selectable_label(fields.selected == Some(index), name)
                                .clicked()
                            {
                                fields.selected = Some(index);
                                fields.name = name.clone();
                            }
                        }
                        if names.is_empty() {
                            ui.weak(tr("No variables"));
                        }
                    });
                });
            ui.vertical(|ui| {
                let selected = fields.selected.filter(|i| *i < names.len());
                if ui
                    .add_enabled(selected.is_some(), egui::Button::new(tr("Remove")))
                    .clicked()
                {
                    action = Some(0);
                }
                if ui
                    .add_enabled(
                        selected.is_some_and(|i| i > 0),
                        egui::Button::new(tr("Move Up")),
                    )
                    .clicked()
                {
                    action = Some(1);
                }
                if ui
                    .add_enabled(
                        selected.is_some_and(|i| i + 1 < names.len()),
                        egui::Button::new(tr("Move Down")),
                    )
                    .clicked()
                {
                    action = Some(2);
                }
            });
        });
        ui.add_space(12.0);
        ui.label(tr("Variable name"));
        ui.add(
            egui::TextEdit::singleline(&mut fields.name)
                .id_salt(("generator-variable", input))
                .char_limit(32)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(fields.selected.is_some(), egui::Button::new(tr("Rename")))
                .clicked()
            {
                action = Some(3);
            }
            if ui
                .add_enabled(names.len() < limit, egui::Button::new(tr("Add")))
                .clicked()
            {
                action = Some(4);
            }
        });
        if input && !self.analysis.is_expression_mode() {
            ui.weak(tr("The first input is the most significant table bit. Removing an input turns conflicting rows into x."));
        }
        if let Some(action) = action {
            let i = fields.selected.unwrap_or(0);
            let result = match action {
                0 => {
                    let result = self.analysis.remove(input, i);
                    if result.is_ok() {
                        fields.selected = None;
                    }
                    result
                }
                1 | 2 => {
                    let target = if action == 1 { i - 1 } else { i + 1 };
                    let result = self.analysis.swap(input, i, target);
                    if result.is_ok() {
                        fields.selected = Some(target);
                    }
                    result
                }
                3 => self.analysis.rename(input, i, fields.name.clone()),
                _ => {
                    let result = if input {
                        self.analysis.add_input(fields.name.clone())
                    } else {
                        self.analysis.add_output(fields.name.clone())
                    };
                    if result.is_ok() {
                        fields.selected = Some(names.len());
                    }
                    result
                }
            };
            self.result(result);
        }
    }
    fn unmaterialized_table(&mut self, ui: &mut Ui) {
        ui.weak(tr("Expressions are stored without enumerating inputs. Generate a complete table only when needed."));
        if self.analysis.inputs().len() > logisim_core::generation::MAX_TABLE_INPUTS {
            ui.weak(tr(
                "Complete truth tables support at most 12 inputs. The expressions are preserved.",
            ));
        } else if ui
            .add_enabled(!self.computing, egui::Button::new(tr("Create Truth Table")))
            .clicked()
        {
            self.request_mode(false);
        }
    }
    fn table(&mut self, ui: &mut Ui, height: f32) {
        if self.analysis.is_expression_mode() {
            self.unmaterialized_table(ui);
            return;
        }
        ui.weak(tr("Click an output: 0 → 1 → x. x is a don't-care and will become 0 or 1 in the generated circuit."));
        let mut edit = None;
        let columns = self.analysis.inputs().len() + self.analysis.outputs().len();
        let cell_width = 64.0;
        let width = columns as f32 * (cell_width + ui.spacing().item_spacing.x);
        egui::ScrollArea::both()
            .id_salt("truth-table")
            .max_height((height - 24.0).max(60.0))
            .show_rows(ui, 24.0, self.analysis.rows() + 1, |ui, range| {
                ui.set_min_width(width);
                for display_row in range {
                    ui.push_id(display_row, |ui| {
                        ui.horizontal(|ui| {
                            if display_row == 0 {
                                for name in self
                                    .analysis
                                    .inputs()
                                    .iter()
                                    .map(String::as_str)
                                    .chain(self.analysis.outputs().iter().map(|o| o.name()))
                                {
                                    ui.add_sized(
                                        [cell_width, 24.0],
                                        egui::Label::new(egui::RichText::new(name).strong())
                                            .truncate(),
                                    )
                                    .on_hover_text(name);
                                }
                                return;
                            }
                            let row = display_row - 1;
                            for input in 0..self.analysis.inputs().len() {
                                let text = if self.analysis.input_value(row, input).unwrap() {
                                    "1"
                                } else {
                                    "0"
                                };
                                ui.add_sized(
                                    [cell_width, 24.0],
                                    egui::Label::new(egui::RichText::new(text).monospace()),
                                );
                            }
                            for (output, column) in self.analysis.outputs().iter().enumerate() {
                                let cell = column.values()[row];
                                if ui
                                    .add_sized([cell_width, 24.0], egui::Button::new(cell.text()))
                                    .clicked()
                                {
                                    edit = Some((output, row, cell.next()));
                                }
                            }
                        })
                    });
                }
            });
        if let Some((output, row, cell)) = edit {
            let result = self.analysis.set_cell(output, row, cell);
            self.result(result);
        }
    }
    fn output_selector(&mut self, ui: &mut Ui) -> bool {
        if self.analysis.outputs().is_empty() {
            ui.weak(tr("Add at least one output before building a circuit."));
            return false;
        }
        let previous = self.output;
        ui.horizontal(|ui| {
            ui.label(tr("Output"));
            ui.add_enabled_ui(!self.pending(), |ui| {
                egui::ComboBox::from_id_salt("generator-output")
                    .selected_text(self.analysis.outputs()[self.output].name())
                    .show_ui(ui, |ui| {
                        for (index, output) in self.analysis.outputs().iter().enumerate() {
                            ui.selectable_value(&mut self.output, index, output.name());
                        }
                    });
            });
        });
        if self.output != previous {
            self.refresh();
        }
        true
    }
    fn expression(&mut self, ui: &mut Ui) {
        if !self.output_selector(ui) {
            return;
        }
        ui.label(tf!("Applied: {}", self.applied));
        self.simplify_controls(ui);
        ui.add_enabled(
            !self.computing,
            egui::TextEdit::multiline(&mut self.draft)
                .id_salt("generator-expression")
                .code_editor()
                .char_limit(MAX_EXPRESSION_BYTES)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.weak(tr(
            "NOT: ~A, !A, A' · AND: A & B, A B · XOR: A ^ B · OR: A | B, A + B",
        ));
        ui.weak(tr(
            "Precedence: NOT > AND > XOR > OR. Use parentheses to group; constants are 0 and 1.",
        ));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.computing, egui::Button::new(tr("Clear")))
                .clicked()
            {
                self.draft.clear();
                self.error = None;
            }
            if ui
                .add_enabled(self.pending(), egui::Button::new(tr("Revert")))
                .clicked()
            {
                self.refresh();
            }
            if ui
                .add_enabled(
                    self.pending() && !self.computing,
                    egui::Button::new(tr("Apply")),
                )
                .clicked()
            {
                let result = self.analysis.apply_expression(self.output, &self.draft);
                self.result(result);
            }
        });
    }
    fn simplify_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!self.computing, |ui| {
                    if ui.button(tr("Simplify")).clicked() {
                        self.request_simplify();
                    }
                    egui::ComboBox::from_id_salt("generator-form")
                        .width(150.0)
                        .selected_text(tr(match self.form {
                            Form::SumOfProducts => "Sum of products",
                            Form::ProductOfSums => "Product of sums",
                        }))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.form,
                                Form::SumOfProducts,
                                tr("Sum of products"),
                            );
                            ui.selectable_value(
                                &mut self.form,
                                Form::ProductOfSums,
                                tr("Product of sums"),
                            );
                        });
                    ui.label(tr("Format"));
                });
            });
        });
    }
}
impl Workspace {
    pub(crate) fn open_generator(&mut self) {
        self.finish_edit();
        if !self.truth_table_generation {
            self.generator.hide_truth_table();
        }
        if self.project().named(&self.generator.name).is_some() {
            let mut i = 2;
            while self.project().named(&format!("generated{i}")).is_some() {
                i += 1;
            }
            self.generator.name = format!("generated{i}");
        }
        self.generator.open = true;
    }
}
impl Editor {
    fn install_generated(&mut self, name: String, circuit: Circuit) -> Result<(), Error> {
        let before = self.snapshot();
        let mut project = before.project.clone();
        let id = project.add(name).map_err(|message| Error {
            message: "Could not create circuit:",
            detail: tr(&message).to_owned(),
        })?;
        project.circuit_mut(id).unwrap().circuit = circuit;
        self.commit_generated(before, project, id)
    }
    fn install_rebuilt(&mut self, target: RebuildTarget, circuit: Circuit) -> Result<(), Error> {
        let before = self.snapshot();
        let mut project = before.project.clone();
        let definition = project.circuit_mut(target.id()).ok_or_else(|| Error {
            message: "The circuit changed during analysis. Please analyze it again.",
            detail: String::new(),
        })?;
        definition.circuit = target.rebuild(definition, circuit)?;
        self.commit_generated(before, project, target.id())
    }
    fn commit_generated(
        &mut self,
        before: crate::editor::Snapshot,
        mut project: logisim_core::Project,
        id: logisim_core::Id,
    ) -> Result<(), Error> {
        if project
            .circuits
            .iter()
            .map(|d| d.circuit.components.len() + d.circuit.wires.len())
            .sum::<usize>()
            > logisim_core::MAX_PROJECT_ITEMS
        {
            return Err(Error {
                message: "The generated circuit would exceed the project item limit.",
                detail: String::new(),
            });
        }
        project.refresh_interfaces(false);
        project.validate().map_err(|message| Error {
            message: "Could not create circuit:",
            detail: tr(&message).to_owned(),
        })?;
        self.project = project;
        // Synchronize the active copy before navigation/recording can write it back.
        self.circuit = self.project.circuit(self.active).unwrap().circuit.clone();
        if self.active != id {
            self.switch_circuit(id);
        } else {
            self.appearance_mode = false;
            self.selection.clear();
            self.set_tool(crate::editor::Tool::Select);
            self.simulation.view = Default::default();
            self.simulation.plugin_arguments.clear();
        }
        self.record(before);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod rebuild_tests;

#[cfg(test)]
mod multi_tests;
