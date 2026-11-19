//! Project navigation and the resizable lower inspector.
use crate::{
    Workspace,
    i18n::tr,
    icons::{self, Icon},
    project::CircuitCommand,
    properties, theme,
};
use egui::{Frame, Margin, Panel, RichText, ScrollArea, Ui, vec2};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Circuits,
    Components,
}

#[derive(Default)]
pub(crate) struct Navigation {
    tab: Tab,
    query: String,
}

impl Workspace {
    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        Panel::bottom("zoom")
            .exact_size(40.0)
            .frame(
                Frame::NONE
                    .fill(theme::CHROME)
                    .inner_margin(Margin::symmetric(8, 5)),
            )
            .show(ui, |ui| self.zoom_controls(ui));

        let height = ui.available_height();
        Panel::top("project")
            .resizable(true)
            .default_size(height * 0.52)
            .size_range(144.0..=(height - (height * 0.45).clamp(126.0, 180.0)).max(144.0))
            .frame(
                Frame::NONE
                    .fill(theme::SURFACE)
                    .inner_margin(Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                self.document_header(ui);
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 4.0) / 2.0;
                    for (tab, label) in
                        [(Tab::Circuits, "Circuits"), (Tab::Components, "Components")]
                    {
                        if ui
                            .add_sized(
                                [width, 28.0],
                                egui::Button::selectable(self.navigation.tab == tab, tr(label)),
                            )
                            .clicked()
                        {
                            self.navigation.tab = tab;
                        }
                    }
                });
                let components = self.navigation.tab == Tab::Components;
                let query = self.navigation.query.trim().to_lowercase();
                let count = self.result_count(components, &query);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                    icons::paint(ui.painter(), rect.shrink(1.0), Icon::Search, theme::MUTED);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.navigation.query)
                            .id_salt("catalogue-search")
                            .hint_text(tr("Search · Ctrl+F"))
                            .desired_width((ui.available_width() - 42.0).max(50.0)),
                    );
                    if ui.is_enabled()
                        && !ui.ctx().any_popup_open()
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F))
                    {
                        response.request_focus();
                    }
                    theme::chip(ui, &count.to_string()).on_hover_text(tr("Results"));
                });
                ui.separator();
                ScrollArea::vertical()
                    .id_salt(("catalogue-scroll", components))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if count == 0 {
                            ui.add_space(4.0);
                            ui.label(tr("No results"));
                            if ui.button(tr("Clear search")).clicked() {
                                self.navigation.query.clear();
                            }
                        } else if components {
                            self.components(ui, &query);
                        } else {
                            self.circuits(ui, &query);
                        }
                    });
            });
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme::CHROME))
            .show(ui, |ui| {
                if self.editor.session.context.appearance_mode {
                    self.appearance.properties(ui, &mut self.editor);
                } else {
                    let name = self.circuit_name().to_owned();
                    properties::show(
                        ui,
                        &mut self.editor,
                        &name,
                        &self.plugins,
                        &mut self.file_actions,
                    );
                }
            });
    }

    fn document_header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(20.0, 24.0), egui::Sense::hover());
            icons::paint(
                ui.painter(),
                rect.shrink2(vec2(2.0, 4.0)),
                Icon::Document,
                theme::ACCENT,
            );
            if self.document.dirty {
                let (r, response) = ui.allocate_exact_size(vec2(8.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 3.0, theme::WARNING);
                response.on_hover_text(tr("Unsaved changes"));
            }
            let title = format!(
                "{}{}",
                self.document.display_title(),
                if self.document.partial {
                    tr(" (partial)")
                } else {
                    ""
                }
            );
            ui.allocate_ui_with_layout(
                vec2(ui.available_width() - 36.0, 28.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(&title).size(15.0).strong()).truncate())
                        .on_hover_text(&title);
                },
            );
            let button = icons::button(ui, Icon::More, false, true, "Project actions");
            egui::Popup::menu(&button).show(|ui| {
                for (name, command) in [
                    ("Add circuit", CircuitCommand::Add),
                    ("Rename…", CircuitCommand::Rename),
                    ("Move up", CircuitCommand::Up),
                    ("Move down", CircuitCommand::Down),
                    ("Remove circuit", CircuitCommand::Remove),
                ] {
                    let enabled = !matches!(command, CircuitCommand::Remove)
                        || self.project().can_remove(self.active_circuit());
                    if ui
                        .add_enabled(enabled, egui::Button::new(tr(name)))
                        .clicked()
                    {
                        self.circuit_command(command);
                        ui.close();
                    }
                }
                if !self.editor.session.context.path.is_empty()
                    && ui
                        .button(tr("Back"))
                        .on_hover_text(tr("Return to parent instance · Alt+Left"))
                        .clicked()
                {
                    self.editor.leave_instance();
                    ui.close();
                }
            });
        });
        if !self.editor.session.context.path.is_empty() {
            ui.weak(self.editor.context_label()).on_hover_text(tr(
                "Viewing this instance. Edits change the shared circuit definition.",
            ));
        }
    }
}
