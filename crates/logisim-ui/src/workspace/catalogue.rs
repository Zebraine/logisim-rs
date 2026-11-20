//! Presentation of circuit definitions and placement tools.
use super::library::groups;
use crate::{
    Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, ComponentIcon, Icon},
    plugins,
    project::CircuitCommand,
    tf, theme,
};
use egui::{Align2, FontId, Rect, Response, Sense, Ui, Vec2, pos2, vec2};
use logisim_core::{Id, Kind};

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
    let inline_width = width - leading - badge_width - 16.;
    let stacked = !badge.is_empty() && inline_width < 64.;
    let text_width = if stacked {
        width - leading - 4.
    } else {
        inline_width
    };
    let badge_size = vec2(badge_width, 20.);
    let run = ui
        .painter()
        .layout(name.to_owned(), font, theme::INK, text_width.max(24.));
    let content_height = run.size().y + if stacked { badge_size.y + 2. } else { 0. };
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, height.max(content_height + 4.)), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::ListBoxOption, ui.is_enabled(), selected, name)
    });
    if ui.is_rect_visible(rect) {
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                4,
                if selected {
                    theme::colors(ui.ctx()).soft
                } else {
                    theme::CHROME
                },
            );
        }
        if selected {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min + vec2(0.0, 4.0), vec2(3.0, rect.height() - 8.0)),
                2,
                theme::colors(ui.ctx()).base,
            );
        }
        theme::focus_ring(ui, &response);
        ui.painter().galley(
            pos2(rect.left() + leading, rect.center().y - content_height / 2.),
            run,
            theme::INK,
        );
        if !badge.is_empty() {
            let r = if stacked {
                Align2::LEFT_BOTTOM.anchor_size(
                    pos2(rect.left() + leading, rect.center().y + content_height / 2.),
                    badge_size,
                )
            } else {
                Align2::RIGHT_CENTER.anchor_size(rect.right_center() - vec2(4., 0.), badge_size)
            };
            ui.painter().rect_filled(
                r,
                4,
                if selected {
                    theme::colors(ui.ctx()).selected
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

fn component_row(ui: &mut Ui, name: &str, selected: bool, icon: ComponentIcon<'_>) -> Response {
    let response = row(
        ui,
        name,
        selected,
        38.0,
        theme::metrics(ui.ctx()).control,
        "",
    );
    if ui.is_rect_visible(response.rect) {
        let center = response.rect.left_center() + vec2(25.0, 0.0);
        let rect = Rect::from_center_size(center, Vec2::splat(18.0));
        icon.paint(
            ui.painter(),
            rect,
            if selected {
                theme::colors(ui.ctx()).base
            } else {
                theme::MUTED
            },
        );
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
    let response = row(ui, name, false, 38.0, theme::metrics(ui.ctx()).row, badge);
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
                            component_row(
                                ui,
                                tr(kind.name()),
                                self.editor.tool == tool,
                                ComponentIcon::Builtin(kind),
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
                            component_row(ui, &local(&t.name), active, ComponentIcon::Plugin(t))
                        })
                        .inner
                        .on_hover_text(tf!("{} · {} ports", &name, t.ports.len()))
                        .clicked()
                    {
                        self.editor.session.context.appearance_mode = false;
                        self.editor.set_plugin_tool(m.components().swap_remove(i));
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
        let d = self
            .editor
            .session
            .project
            .circuit(id)
            .expect("listed circuit");
        let active = self.active_circuit() == id;
        let response = ui
            .push_id(id, |ui| {
                row(
                    ui,
                    &d.name,
                    active,
                    72.0,
                    crate::drawing::thumbnails::SIZE.y + f32::from(theme::metrics(ui.ctx()).gap),
                    if self.project().main == id {
                        tr("Main circuit")
                    } else {
                        ""
                    },
                )
            })
            .inner;
        if ui.is_rect_visible(response.rect) {
            let preview = Rect::from_center_size(
                response.rect.left_center() + vec2(36.0, 0.0),
                crate::drawing::thumbnails::SIZE,
            );
            if self
                .editor
                .thumbnails
                .paint(ui.painter(), id, &d.circuit, preview, active)
            {
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(preview.center(), vec2(16.0, 16.0)),
                    Icon::Chip,
                    theme::WARNING,
                );
                response.clone().on_hover_text(tr("Missing plugin"));
            }
        }
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
