//! Project overview reads the live document and dispatches edits after drawing its rows.
use crate::{
    FileAction, Workspace,
    dialog::{Dialog, Dismiss},
    i18n::tr,
    theme,
};
use egui::{Align, Context, Label, Layout, Ui, vec2};
use logisim_core::Id;

enum Action {
    Open(Id),
    SetMain(Id),
    LoadPlugin,
}

impl Workspace {
    pub(super) fn project_options(&mut self, ctx: &Context) {
        let mut action = None;
        let dismiss = Dialog::editor("project-options", 780., 640.).show(
            ctx,
            tr("Project Options"),
            |dialog| {
                dialog.body(|ui| {
                    let gap = f32::from(theme::metrics(ctx).gap);
                    theme::card(ui, tr("Summary"), |ui| self.project_summary(ui));
                    ui.add_space(gap);
                    theme::card(ui, tr("Circuits"), |ui| self.circuit_list(ui, &mut action));
                    ui.add_space(gap);
                    theme::card(ui, tr("Plugin dependencies"), |ui| {
                        self.dependency_list(ui, &mut action)
                    });
                    ui.add_space(gap);
                    theme::card(ui, tr("Imported libraries"), |ui| {
                        self.library_list(ui);
                    });
                });
                dialog.footer("Close", |_| {});
            },
        );
        match action {
            Some(Action::Open(id)) => {
                self.editor.switch_circuit(id);
                self.project_options_open = false;
            }
            Some(Action::SetMain(id)) => self.set_main_circuit(id),
            Some(Action::LoadPlugin) => {
                self.file_actions.push(FileAction::LoadPlugin);
                self.project_options_open = false;
            }
            None => self.project_options_open = dismiss == Dismiss::Open,
        }
    }

    fn imported_library_counts(&self) -> impl Iterator<Item = (&str, usize)> {
        // The palette retains group IDs for redo. Only extant definitions belong in the overview.
        self.libraries.iter().filter_map(|group| {
            let count = group
                .circuits
                .iter()
                .filter(|id| self.project().circuit(**id).is_some())
                .count();
            (count > 0).then_some((group.name.as_str(), count))
        })
    }

    fn project_summary(&self, ui: &mut Ui) {
        let project = self.project();
        let path = self.document.path.as_deref().map(|p| p.to_string_lossy());
        let status = tr(if self.document.dirty {
            "Unsaved changes"
        } else {
            "No changes"
        });
        row_with_end(
            ui,
            |ui| {
                field(
                    ui,
                    "Project path",
                    path.as_deref().unwrap_or(tr("Untitled")),
                )
            },
            |ui| {
                theme::chip(ui, status);
            },
        );
        ui.columns(2, |cols| {
            for (col, (label, name)) in cols.iter_mut().zip([
                ("Active circuit", self.circuit_name()),
                ("Main circuit", &project.main_circuit().name),
            ]) {
                col.horizontal(|ui| field(ui, label, name));
            }
        });
        ui.separator();
        let (components, wires) = project.circuits.iter().fold((0, 0), |(parts, nets), d| {
            (
                parts + d.circuit.components.len(),
                nets + d.circuit.wires.len(),
            )
        });
        let counts = [
            ("Circuits", project.circuits.len()),
            ("Components", components),
            ("Wires", wires),
            ("Plugins", project.plugin_sources.len()),
            ("Libraries", self.imported_library_counts().count()),
        ];
        ui.columns(counts.len(), |cols| {
            for (col, (label, count)) in cols.iter_mut().zip(counts) {
                col.weak(tr(label));
                col.strong(count.to_string());
            }
        });
    }

    fn circuit_list(&self, ui: &mut Ui, action: &mut Option<Action>) {
        let spacing = ui.spacing().item_spacing.x;
        let components_width = 80.;
        let wires_width = 46.;
        let actions_width = 134.;
        let name_width =
            (ui.available_width() - components_width - wires_width - actions_width - 3. * spacing)
                .max(1.);
        ui.horizontal(|ui| {
            cell(ui, name_width, |ui| {
                ui.weak(tr("Circuit name"));
            });
            cell(ui, components_width, |ui| {
                ui.weak(tr("Components"));
            });
            cell(ui, wires_width, |ui| {
                ui.weak(tr("Wires"));
            });
        });
        for d in &self.project().circuits {
            ui.push_id(d.id, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    let main = d.id == self.project().main;
                    cell(ui, name_width, |ui| {
                        row_with_end(
                            ui,
                            |ui| clipped(ui, &d.name),
                            |ui| {
                                if d.id == self.active_circuit() {
                                    theme::chip(ui, tr("Active"));
                                }
                                if main {
                                    theme::chip(ui, tr("Main"));
                                }
                            },
                        );
                    });
                    cell(ui, components_width, |ui| {
                        ui.label(d.circuit.components.len().to_string());
                    });
                    cell(ui, wires_width, |ui| {
                        ui.label(d.circuit.wires.len().to_string());
                    });
                    if ui.button(tr("Open")).clicked() {
                        *action = Some(Action::Open(d.id));
                    }
                    if ui
                        .add_enabled(!main, egui::Button::new(tr("Set as Main")))
                        .clicked()
                    {
                        *action = Some(Action::SetMain(d.id));
                    }
                });
            });
        }
    }

    fn dependency_list(&self, ui: &mut Ui, action: &mut Option<Action>) {
        let sources = &self.project().plugin_sources;
        if sources.is_empty() {
            ui.weak(tr("No plugin dependencies."));
        }
        for (i, source) in sources.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let loaded = self.plugins.iter().any(|lib| {
                lib.manifest.id == source.plugin && lib.manifest.version == source.version
            });
            row_with_end(
                ui,
                |ui| {
                    cell(ui, ui.available_width() * 0.65, |ui| {
                        clipped(ui, &source.plugin)
                    });
                    clipped(ui, &source.version);
                },
                |ui| {
                    theme::chip(ui, tr(if loaded { "Loaded" } else { "Missing" }));
                },
            );
            ui.horizontal(|ui| field(ui, "Path", &source.path));
            if !source.fallback.is_empty() {
                ui.horizontal(|ui| field(ui, "Fallback path", &source.fallback));
            }
        }
        if ui.button(tr("Load Plugin…")).clicked() {
            *action = Some(Action::LoadPlugin);
        }
    }
    fn library_list(&self, ui: &mut Ui) {
        let mut libraries = self.imported_library_counts().peekable();
        if libraries.peek().is_none() {
            ui.weak(tr("No imported libraries."));
        }
        for (name, count) in libraries {
            row_with_end(
                ui,
                |ui| clipped(ui, name),
                |ui| {
                    ui.label(crate::tf!("Circuits: {}", count));
                },
            );
        }
    }
}

fn cell(ui: &mut Ui, width: f32, body: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        vec2(width, ui.spacing().interact_size.y),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_width(width);
            body(ui);
        },
    );
}

// Lay out the trailing controls first so long document strings use only the remaining space.
fn row_with_end(ui: &mut Ui, start: impl FnOnce(&mut Ui), end: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            end(ui);
            ui.with_layout(Layout::left_to_right(Align::Center), start);
        });
    });
}

fn field(ui: &mut Ui, label: &str, value: &str) {
    ui.weak(tr(label));
    clipped(ui, value);
}

fn clipped(ui: &mut Ui, text: &str) {
    ui.add(Label::new(text).truncate());
}
