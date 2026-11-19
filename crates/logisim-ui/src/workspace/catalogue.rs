//! Presentation of circuit definitions and placement tools.
use super::library::groups;
use crate::{
    Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, Icon},
    plugins,
    project::CircuitCommand,
    tf, theme,
};
use egui::{Align2, FontId, Rect, Response, Sense, Ui, pos2, vec2};
use logisim_core::{Component, Id, Kind, Point};

fn matches(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

/// Reserve one focusable hit target for a complete row, including its visual accessories.
fn row(
    ui: &mut Ui,
    name: &str,
    selected: bool,
    leading: f32,
    height: f32,
    badge: &str,
) -> Response {
    let font = FontId::proportional(13.0);
    let badge_run =
        ui.painter()
            .layout_no_wrap(badge.to_owned(), FontId::proportional(12.0), theme::MUTED);
    let badge_width = if badge.is_empty() {
        0.0
    } else {
        badge_run.size().x + 12.0
    };
    let width = ui.available_width();
    let run = ui.painter().layout(
        name.to_owned(),
        font,
        theme::INK,
        (width - leading - badge_width - 16.0).max(24.0),
    );
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, height.max(run.size().y + 4.0)), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::ListBoxOption, ui.is_enabled(), selected, name)
    });
    if ui.is_rect_visible(rect) {
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                4,
                if selected {
                    theme::ACCENT_SOFT
                } else {
                    theme::CHROME
                },
            );
        }
        if selected {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min + vec2(0.0, 4.0), vec2(3.0, rect.height() - 8.0)),
                2,
                theme::ACCENT,
            );
        }
        theme::focus_ring(ui, &response);
        ui.painter().galley(
            pos2(rect.left() + leading, rect.center().y - run.size().y / 2.0),
            run,
            theme::INK,
        );
        if !badge.is_empty() {
            let r = Align2::RIGHT_CENTER.anchor_size(
                rect.right_center() - vec2(4.0, 0.0),
                vec2(badge_width, 20.0),
            );
            ui.painter().rect_filled(
                r,
                4,
                if selected {
                    theme::SELECTED
                } else {
                    theme::SUNKEN
                },
            );
            ui.painter()
                .galley(r.center() - badge_run.size() / 2.0, badge_run, theme::MUTED);
        }
    }
    response
}

fn group(
    ui: &mut Ui,
    key: impl egui::AsIdSalt,
    name: &str,
    badge: &str,
    icon: Icon,
    searching: bool,
) -> (bool, Response) {
    let id = ui.make_persistent_id(key);
    let opened = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let response = row(ui, name, false, 38.0, 30.0, badge);
    let open = opened ^ response.clicked();
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    let expanded = open || searching;
    let p = response.rect.left_center() + vec2(8.0, 0.0);
    let arrow = if expanded {
        [vec2(-3.0, -2.0), vec2(0.0, 1.0), vec2(3.0, -2.0)]
    } else {
        [vec2(-1.0, -3.0), vec2(2.0, 0.0), vec2(-1.0, 3.0)]
    };
    ui.painter().add(egui::Shape::line(
        arrow.iter().map(|v| p + *v).collect(),
        egui::Stroke::new(1.5, theme::MUTED),
    ));
    icons::paint(
        ui.painter(),
        Rect::from_center_size(p + vec2(17.0, 0.0), vec2(15.0, 15.0)),
        icon,
        theme::MUTED,
    );
    (expanded, response)
}

impl Workspace {
    pub(super) fn result_count(&self, components: bool, q: &str) -> usize {
        if !components {
            return self
                .project()
                .circuits
                .iter()
                .filter(|d| matches(&d.name, q))
                .count();
        }
        let native = groups()
            .iter()
            .map(|g| {
                g.tools
                    .iter()
                    .filter(|k| matches(tr(k.name()), q) || matches(tr(g.key), q))
                    .count()
            })
            .sum::<usize>();
        native
            + self
                .plugins
                .iter()
                .map(|l| {
                    l.manifest
                        .tools
                        .iter()
                        .filter(|t| {
                            matches(plugins::text(Some(&l.presentation), &t.name), q)
                                || matches(
                                    plugins::text(Some(&l.presentation), &l.manifest.name),
                                    q,
                                )
                        })
                        .count()
                })
                .sum::<usize>()
    }

    pub(super) fn components(&mut self, ui: &mut Ui, q: &str) {
        for (g, icon) in groups().iter().zip([
            Icon::Wire,
            Icon::And,
            Icon::Tunnel,
            Icon::Plus,
            Icon::Chip,
            Icon::Pin,
            Icon::Text,
        ]) {
            let filter = |k: &&Kind| matches(tr(k.name()), q) || matches(tr(g.key), q);
            let count = g.tools.iter().filter(filter).count();
            if count == 0 {
                continue;
            }
            let (expanded, _) = group(
                ui,
                g.key,
                tr(g.key),
                &count.to_string(),
                icon,
                !q.is_empty(),
            );
            if expanded {
                for &kind in g.tools.iter().filter(filter) {
                    let tool = Tool::Place(kind);
                    if ui
                        .push_id(kind.name(), |ui| {
                            row(
                                ui,
                                tr(kind.name()),
                                self.editor.tool == tool,
                                38.0,
                                28.0,
                                "",
                            )
                        })
                        .inner
                        .on_hover_text(tf!("{} · Click to place", tr(g.key)))
                        .clicked()
                    {
                        self.editor.session.context.appearance_mode = false;
                        self.editor.set_tool(tool);
                    }
                }
            }
        }
        for lib in &self.plugins {
            let m = &lib.manifest;
            let local = |s: &str| {
                lib.presentation
                    .text(crate::i18n::current_language().code(), s)
                    .to_owned()
            };
            let name = local(&m.name);
            let filter = |t: &&logisim_core::plugin::api::Tool| {
                matches(&name, q) || matches(&local(&t.name), q)
            };
            let count = m.tools.iter().filter(filter).count();
            if count == 0 {
                continue;
            }
            let (expanded, header) = group(
                ui,
                (&m.id, &m.version),
                &name,
                &format!("v{} · {}", m.version, count),
                Icon::Chip,
                !q.is_empty(),
            );
            header.on_hover_ui(|ui| {
                ui.label(&m.id);
                if let Some(source) = self
                    .editor
                    .session
                    .project
                    .plugin_sources
                    .iter()
                    .find(|s| s.plugin == m.id && s.version == m.version)
                {
                    ui.label(&source.path);
                }
            });
            if expanded {
                for (i, t) in m.tools.iter().enumerate().filter(|(_, t)| filter(t)) {
                    let active = self.editor.tool == Tool::Place(Kind::Plugin)
                        && self.editor.template.plugin.as_ref().is_some_and(|p| {
                            p.plugin == m.id && p.version == m.version && p.tool.id == t.id
                        });
                    if ui
                        .push_id((&m.id, &m.version, &t.id), |ui| {
                            row(ui, &local(&t.name), active, 38.0, 28.0, "")
                        })
                        .inner
                        .on_hover_text(tf!("{} · {} ports", &name, t.ports.len()))
                        .clicked()
                    {
                        self.editor.session.context.appearance_mode = false;
                        self.editor.set_tool(Tool::Place(Kind::Plugin));
                        self.editor.template =
                            Component::from_plugin(m.components().swap_remove(i), Point::ZERO);
                    }
                }
            }
        }
    }

    pub(super) fn circuits(&mut self, ui: &mut Ui, q: &str) {
        let definitions: Vec<_> = self
            .project()
            .circuits
            .iter()
            .filter(|d| {
                matches(&d.name, q) && !self.libraries.iter().any(|g| g.circuits.contains(&d.id))
            })
            .map(|d| d.id)
            .collect();
        for id in definitions {
            self.circuit_row(ui, id);
        }
        for index in 0..self.libraries.len() {
            let g = &self.libraries[index];
            let ids: Vec<_> = g
                .circuits
                .iter()
                .copied()
                .filter(|id| {
                    self.project()
                        .circuit(*id)
                        .is_some_and(|d| matches(&d.name, q))
                })
                .collect();
            if ids.is_empty() {
                continue;
            }
            let (expanded, header) = group(
                ui,
                ("imported", g.circuits[0]),
                &g.name,
                tr("Imported"),
                Icon::Document,
                !q.is_empty(),
            );
            header.on_hover_text(tr("Embedded library snapshot"));
            if expanded {
                for id in ids {
                    self.circuit_row(ui, id);
                }
            }
        }
    }

    fn circuit_row(&mut self, ui: &mut Ui, id: Id) {
        let d = self.project().circuit(id).expect("listed circuit");
        let active = self.active_circuit() == id;
        let response = ui
            .push_id(id, |ui| {
                row(
                    ui,
                    &d.name,
                    active,
                    28.0,
                    30.0,
                    if self.project().main == id {
                        tr("Main circuit")
                    } else {
                        ""
                    },
                )
            })
            .inner;
        icons::paint(
            ui.painter(),
            Rect::from_center_size(
                response.rect.left_center() + vec2(16.0, 0.0),
                vec2(14.0, 14.0),
            ),
            Icon::Document,
            theme::MUTED,
        );
        let response = response.on_hover_text(tr(
            "Double-click to edit; single-click another circuit to place an instance.",
        ));
        if response.double_clicked() {
            self.editor.switch_circuit(id);
        } else if response.clicked() && self.project().can_instantiate(self.active_circuit(), id) {
            self.editor.session.context.appearance_mode = false;
            self.editor.set_tool(Tool::Place(Kind::Subcircuit(id)));
        }
        response.context_menu(|ui| {
            if ui.button(tr("Open circuit")).clicked() {
                self.editor.switch_circuit(id);
                ui.close();
            }
            if ui.button(tr("Analyze Circuit…")).clicked() {
                self.request_circuit_analysis(id);
                ui.close();
            }
            if ui
                .add_enabled(
                    self.project().can_instantiate(self.active_circuit(), id),
                    egui::Button::new(tr("Place instance")),
                )
                .clicked()
            {
                self.editor.set_tool(Tool::Place(Kind::Subcircuit(id)));
                ui.close();
            }
            if ui.button(tr("Edit appearance")).clicked() {
                self.finish_edit();
                self.editor.switch_circuit(id);
                self.editor.session.context.appearance_mode = true;
                ui.close();
            }
            if ui.button(tr("Rename…")).clicked() {
                self.editor.switch_circuit(id);
                self.circuit_command(CircuitCommand::Rename);
                ui.close();
            }
        });
    }
}
