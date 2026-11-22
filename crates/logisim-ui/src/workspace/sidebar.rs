//! Project navigation and the resizable lower inspector.
use crate::shortcuts::ChordUi as _;
use crate::shortcuts::ShortcutCaptions as _;
use crate::{
    Workspace,
    i18n::tr,
    icons::{self, Icon},
    project::CircuitCommand,
    properties, theme,
};
use egui::{Frame, Margin, Panel, RichText, ScrollArea, Ui, vec2};

#[derive(Default)]
pub(crate) struct Navigation {
    pub(super) last_height: f32,
    pub(super) last_width: f32,
    circuit_query: String,
    component_query: String,
}

impl Workspace {
    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        let m = theme::metrics(ui.ctx());
        let zoom_margin = if ui.available_width() < m.sidebar.min {
            m.gap
        } else {
            m.card_margin
        };
        Panel::bottom("zoom")
            .exact_size(m.zoom_bar)
            .frame(
                Frame::NONE
                    .fill(theme::palette(ui.ctx()).chrome)
                    .inner_margin(Margin::symmetric(
                        zoom_margin,
                        ((m.zoom_bar - m.control) / 2.) as i8,
                    )),
            )
            .show(ui, |ui| self.zoom_controls(ui));

        let height = ui.available_height();
        let min = (height * 0.25).max(144.).min(height * 0.52);
        let max = (height * 0.75).min(height - 100.).max(min);
        let mut size = (height * self.layout.split).clamp(min, max);
        if self.layout.tab == super::LayoutTab::Circuits
            && self.layout.split == super::WorkspaceLayout::default().split
            && self.libraries.is_empty()
        {
            let header = 3. * (m.control + f32::from(m.gap)) + 18.;
            let row = crate::drawing::thumbnails::SIZE.y + 2. * f32::from(m.gap);
            size = size
                .min(header + self.project().circuits.len() as f32 * row)
                .max(min);
        }
        let panel =
            super::layout::prepare_panel(ui, "project", size, &mut self.navigation.last_height);
        Panel::top("project")
            .resizable(true)
            .default_size(size)
            .size_range(min..=max)
            .frame(
                Frame::NONE
                    .fill(theme::palette(ui.ctx()).surface)
                    .inner_margin(Margin::symmetric(m.card_margin, m.gap)),
            )
            .show(ui, |ui| {
                let mut contents = super::layout::panel_contents(ui);
                // Below one full result row, let the catalogue header scroll with the list.
                let short = contents.available_height()
                    < 3. * (m.control + f32::from(m.gap))
                        + crate::drawing::thumbnails::SIZE.y
                        + 12.;
                if short {
                    ScrollArea::vertical()
                        .id_salt("short-catalogue")
                        .auto_shrink([false, false])
                        .show(&mut contents, |ui| {
                            self.catalogue_panel(ui, false);
                        });
                } else {
                    self.catalogue_panel(&mut contents, true);
                }
            });
        if let Some(new_size) = panel.committed(ui, 1) {
            self.layout.split = (new_size / height).clamp(0.25, 0.75);
        }
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme::palette(ui.ctx()).chrome))
            .show(ui, |ui| {
                if self.editor.session.context.appearance_mode {
                    self.appearance
                        .properties(ui, &mut self.editor, &self.preferences.shortcuts);
                } else {
                    let name = self.circuit_name().to_owned();
                    properties::show(
                        ui,
                        &mut self.editor,
                        &name,
                        &self.plugins,
                        &mut self.file_actions,
                        &self.preferences.shortcuts,
                    );
                }
            });
    }

    fn catalogue_panel(&mut self, ui: &mut Ui, fixed_header: bool) {
        let m = theme::metrics(ui.ctx());
        self.document_header(ui);
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 4.0) / 2.0;
            for (tab, label) in [
                (super::LayoutTab::Circuits, "Circuits"),
                (super::LayoutTab::Components, "Components"),
            ] {
                if ui
                    .add_sized(
                        [width, m.control],
                        egui::Button::selectable(self.layout.tab == tab, tr(label)),
                    )
                    .clicked()
                {
                    self.layout.tab = tab;
                }
            }
        });
        let components = self.layout.tab == super::LayoutTab::Components;
        let (query, count) = ui
            .horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                icons::paint(
                    ui.painter(),
                    rect.shrink(1.0),
                    Icon::Search,
                    theme::palette(ui.ctx()).muted,
                );
                let filter = if components {
                    &mut self.navigation.component_query
                } else {
                    &mut self.navigation.circuit_query
                };
                let response = ui.add(
                    egui::TextEdit::singleline(filter)
                        .id_salt(("catalogue-search", components))
                        .hint_text(tr("Search · Ctrl+F"))
                        .desired_width((ui.available_width() - 42.0).max(50.0)),
                );
                if ui.is_enabled()
                    && !crate::shortcuts::popup_open(ui.ctx())
                    && crate::shortcuts::Chord::consume(ui, egui::Modifiers::COMMAND, egui::Key::F)
                {
                    response.request_focus();
                }
                let query = filter.trim().to_lowercase();
                let count = self.result_count(components, &query);
                theme::chip(ui, &count.to_string()).on_hover_text(tr("Results"));
                (query, count)
            })
            .inner;
        ui.separator();
        let mut results = |ui: &mut Ui| {
            if count == 0 {
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr(if components {
                        "No matching components"
                    } else {
                        "No matching circuits"
                    }));
                    if !query.is_empty() && ui.button(tr("Clear search")).clicked() {
                        if components {
                            self.navigation.component_query.clear();
                        } else {
                            self.navigation.circuit_query.clear();
                        }
                    }
                });
            } else if components {
                self.components(ui, &query);
            } else {
                self.circuits(ui, &query);
            }
        };
        if fixed_header {
            ScrollArea::vertical()
                .id_salt(("catalogue-scroll", components))
                .auto_shrink([false, false])
                .show(ui, results);
        } else {
            results(ui);
        }
    }

    fn document_header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(20.0, 24.0), egui::Sense::hover());
            icons::paint(
                ui.painter(),
                rect.shrink2(vec2(2.0, 4.0)),
                Icon::Document,
                theme::colors(ui.ctx()).base,
            );
            if self.document.dirty {
                let (r, response) = ui.allocate_exact_size(vec2(8.0, 12.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(r.center(), 3.0, theme::palette(ui.ctx()).warning);
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
                vec2(
                    ui.available_width() - theme::metrics(ui.ctx()).icon_button - 8.,
                    theme::metrics(ui.ctx()).control,
                ),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(&title).size(15.0).strong()).truncate())
                        .on_hover_text(&title);
                },
            );
            let button =
                icons::button(ui, Icon::More, false, true).on_hover_text(tr("Project actions"));
            egui::Popup::menu(&button).show(|ui| {
                for (name, command) in [
                    ("Add circuit", CircuitCommand::Add),
                    ("Rename…", CircuitCommand::Rename),
                    ("Move up", CircuitCommand::Up),
                    ("Move down", CircuitCommand::Down),
                    ("Remove circuit", CircuitCommand::Remove),
                ] {
                    if ui
                        .add_enabled(command.enabled(self), egui::Button::new(tr(name)))
                        .clicked()
                    {
                        self.circuit_command(command);
                        ui.close();
                    }
                }
                if !self.editor.session.context.path.is_empty()
                    && ui
                        .button(tr("Back"))
                        .on_hover_text(self.preferences.shortcuts.caption_with(
                            crate::shortcuts::Command::ReturnParent,
                            "Return to parent instance",
                            ui,
                        ))
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
