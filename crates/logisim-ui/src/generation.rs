//! The analyzer is a session draft. Only Build commits an editable circuit.
use crate::{Workspace, editor::Editor, i18n::tr, tf};
use egui::{Color32, Ui};
use logisim_core::{
    Circuit,
    generation::{Analysis, Error, Form, MAX_EXPRESSION_BYTES},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Inputs,
    Outputs,
    Table,
    Expression,
    Minimized,
}

#[derive(Default)]
struct VariableEditor {
    selected: Option<usize>,
    name: String,
}

pub(crate) struct Generator {
    pub open: bool,
    analysis: Analysis,
    tab: Tab,
    inputs: VariableEditor,
    outputs: VariableEditor,
    output: usize,
    draft: String,
    applied: String,
    minimized: String,
    form: Form,
    name: String,
    error: Option<Error>,
}
impl Default for Generator {
    fn default() -> Self {
        let mut analysis = Analysis::default();
        for name in ["A", "B"] {
            analysis.add_input(name.into()).unwrap();
        }
        analysis.add_output("Y".into()).unwrap();
        Self {
            open: false,
            analysis,
            tab: Tab::Inputs,
            inputs: Default::default(),
            outputs: Default::default(),
            output: 0,
            draft: "0".into(),
            applied: "0".into(),
            minimized: "0".into(),
            form: Form::default(),
            name: "generated".into(),
            error: None,
        }
    }
}
impl Generator {
    fn pending(&self) -> bool {
        self.draft != self.applied
    }
    fn refresh(&mut self) {
        self.output = self
            .output
            .min(self.analysis.outputs().len().saturating_sub(1));
        self.applied = self
            .analysis
            .expression(self.output)
            .map(|e| e.text(self.analysis.inputs()))
            .unwrap_or_default();
        self.draft.clone_from(&self.applied);
        self.minimized = self
            .analysis
            .minimized(self.output, self.form)
            .map(|e| e.text(self.analysis.inputs()))
            .unwrap_or_default();
        self.error = None;
    }
    fn result(&mut self, result: Result<(), Error>) {
        match result {
            Ok(()) => self.refresh(),
            Err(error) => self.error = Some(error),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, editor: &mut Editor) {
        if !self.open {
            return;
        }
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("circuit-generator")).show(ctx, |ui| {
            let available = ctx.content_rect().size();
            ui.set_width((available.x - 80.0).clamp(280.0, 760.0));
            ui.heading(tr("Combinational Circuit Generator"));
            ui.weak(tr("Single-bit inputs and outputs · up to 8 each"));
            let pending = self.pending();
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (Tab::Inputs, "Input variables"),
                    (Tab::Outputs, "Output variables"),
                    (Tab::Table, "Truth Table"),
                    (Tab::Expression, "Expression"),
                    (Tab::Minimized, "Minimized"),
                ] {
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
            egui::ScrollArea::vertical()
                .id_salt("generator-body")
                .max_height(height)
                .min_scrolled_height(height)
                .show(ui, |ui| {
                    ui.set_min_height(height);
                    match self.tab {
                        Tab::Inputs => self.variables(ui, true),
                        Tab::Outputs => self.variables(ui, false),
                        Tab::Table => self.table(ui),
                        Tab::Expression => self.expression(ui),
                        Tab::Minimized => self.minimized(ui),
                    }
                });
            ui.separator();
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
                ui.label(tr("New circuit name"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .id_salt("generated-name")
                        .char_limit(1024)
                        .desired_width(200.0),
                );
                if ui
                    .add_enabled(
                        !self.pending() && !self.analysis.outputs().is_empty(),
                        egui::Button::new(tr("Build Circuit")),
                    )
                    .clicked()
                {
                    let result = self.analysis.generate().and_then(|circuit| {
                        editor.install_generated(self.name.trim().to_owned(), circuit)
                    });
                    match result {
                        Ok(()) => close = true,
                        Err(error) => self.error = Some(error),
                    }
                }
                if ui.button(tr("Close")).clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.open = false;
        }
    }
    fn variables(&mut self, ui: &mut Ui, input: bool) {
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
        ui.horizontal(|ui| {
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
                .add_enabled(names.len() < 8, egui::Button::new(tr("Add")))
                .clicked()
            {
                action = Some(4);
            }
        });
        if input {
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
    fn table(&mut self, ui: &mut Ui) {
        ui.weak(tr("Click an output: 0 → 1 → x. x is a don't-care and will become 0 or 1 in the generated circuit."));
        let mut edit = None;
        egui::ScrollArea::horizontal()
            .id_salt("truth-table-horizontal")
            .show(ui, |ui| {
                egui::Grid::new("generator-table")
                    .striped(true)
                    .min_col_width(36.0)
                    .show(ui, |ui| {
                        for name in self
                            .analysis
                            .inputs()
                            .iter()
                            .map(String::as_str)
                            .chain(self.analysis.outputs().iter().map(|o| o.name()))
                        {
                            ui.add_sized(
                                [64.0, 24.0],
                                egui::Label::new(egui::RichText::new(name).strong()).truncate(),
                            )
                            .on_hover_text(name);
                        }
                        ui.end_row();
                        for row in 0..self.analysis.rows() {
                            for input in 0..self.analysis.inputs().len() {
                                ui.monospace(if self.analysis.input_value(row, input).unwrap() {
                                    "1"
                                } else {
                                    "0"
                                });
                            }
                            for (output, column) in self.analysis.outputs().iter().enumerate() {
                                let cell = column.values()[row];
                                if ui
                                    .add_sized([64.0, 24.0], egui::Button::new(cell.text()))
                                    .clicked()
                                {
                                    edit = Some((output, row, cell.next()));
                                }
                            }
                            ui.end_row();
                        }
                    });
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
        ui.add(
            egui::TextEdit::multiline(&mut self.draft)
                .id_salt("generator-expression")
                .code_editor()
                .char_limit(MAX_EXPRESSION_BYTES)
                .desired_rows(5)
                .desired_width(f32::INFINITY),
        );
        ui.weak(tr(
            "NOT: ~A, !A, A' · AND: A & B, A B · XOR: A ^ B · OR: A | B, A + B",
        ));
        ui.weak(tr(
            "Precedence: NOT > AND > XOR > OR. Use parentheses to group; constants are 0 and 1.",
        ));
        ui.horizontal(|ui| {
            if ui.button(tr("Clear")).clicked() {
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
                .add_enabled(self.pending(), egui::Button::new(tr("Apply")))
                .clicked()
            {
                let result = self.analysis.apply_expression(self.output, &self.draft);
                self.result(result);
            }
        });
    }
    fn minimized(&mut self, ui: &mut Ui) {
        if !self.output_selector(ui) {
            return;
        }
        let before = self.form;
        ui.horizontal(|ui| {
            ui.label(tr("Format"));
            egui::ComboBox::from_id_salt("generator-form")
                .selected_text(tr(match self.form {
                    Form::SumOfProducts => "Sum of products",
                    Form::ProductOfSums => "Product of sums",
                }))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.form, Form::SumOfProducts, tr("Sum of products"));
                    ui.selectable_value(&mut self.form, Form::ProductOfSums, tr("Product of sums"));
                });
        });
        if self.form != before {
            self.refresh();
        }
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.minimized.as_str())
                .code_editor()
                .desired_rows(7)
                .desired_width(f32::INFINITY),
        );
        if ui.button(tr("Set As Expression")).clicked() {
            let result = self.analysis.apply_expression(self.output, &self.minimized);
            if result.is_ok() {
                self.tab = Tab::Expression;
            }
            self.result(result);
        }
    }
}
impl Workspace {
    pub(crate) fn open_generator(&mut self) {
        self.finish_edit();
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
        self.switch_circuit(id);
        self.record(before);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
