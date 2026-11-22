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
use logisim_core::{
    Id, Kind,
    plugin::api::{Tool as PluginTool, presentation::Library},
};

fn matches(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

fn builtin_matches(kind: Kind, group: &str, query: &str) -> bool {
    query.is_empty()
        || [kind.name(), tr(kind.name()), group, tr(group)]
            .into_iter()
            .any(|name| matches(name, query))
}

fn plugin_matches(lib: &Library, tool: &PluginTool, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let name = &lib.manifest.name;
    [
        name.as_str(),
        &tool.name,
        &lib.manifest.id,
        &tool.id,
        plugins::text(Some(&lib.presentation), name),
        plugins::text(Some(&lib.presentation), &tool.name),
    ]
    .into_iter()
    .any(|s| matches(s, query))
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
    let paint = theme::palette(ui.ctx());
    let font = FontId::proportional(13.0);
    let badge_ink = if selected && paint.mode == crate::ThemeMode::Dark {
        paint.ink
    } else {
        paint.muted
    };
    let badge_run =
        ui.painter()
            .layout_no_wrap(badge.to_owned(), FontId::proportional(12.0), badge_ink);
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
        .layout(name.to_owned(), font, paint.ink, text_width.max(24.));
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
                    paint.chrome
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
            paint.ink,
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
                    paint.sunken
                },
            );
            ui.painter()
                .galley(r.center() - badge_run.size() / 2.0, badge_run, badge_ink);
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
                theme::palette(ui.ctx()).muted
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
    query: &str,
) -> (bool, Response) {
    let id = ui.make_persistent_id((key, query));
    let opened = ui
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(!query.is_empty());
    let response = row(ui, name, false, 38.0, theme::metrics(ui.ctx()).row, badge);
    let expanded = opened ^ response.clicked();
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(id, expanded));
    }
    let p = response.rect.left_center() + vec2(8.0, 0.0);
    let arrow = if expanded {
        [vec2(-3.0, -2.0), vec2(0.0, 1.0), vec2(3.0, -2.0)]
    } else {
        [vec2(-1.0, -3.0), vec2(2.0, 0.0), vec2(-1.0, 3.0)]
    };
    ui.painter().add(egui::Shape::line(
        arrow.iter().map(|v| p + *v).collect(),
        egui::Stroke::new(1.5, theme::palette(ui.ctx()).muted),
    ));
    icons::paint(
        ui.painter(),
        Rect::from_center_size(p + vec2(17.0, 0.0), vec2(15.0, 15.0)),
        icon,
        theme::palette(ui.ctx()).muted,
    );
    (expanded, response)
}

impl Workspace {
    fn circuit_matches(&self, d: &logisim_core::CircuitDefinition, q: &str) -> bool {
        matches(&d.name, q)
            || self
                .libraries
                .iter()
                .any(|g| g.circuits.contains(&d.id) && matches(&g.name, q))
    }
    pub(super) fn result_count(&self, components: bool, q: &str) -> usize {
        if !components {
            return self
                .project()
                .circuits
                .iter()
                .filter(|d| self.circuit_matches(d, q))
                .count();
        }
        let native = groups()
            .iter()
            .map(|g| {
                g.tools
                    .iter()
                    .filter(|k| builtin_matches(**k, g.key, q))
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
                        .filter(|t| plugin_matches(l, t, q))
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
            let filter = |k: &&Kind| builtin_matches(**k, g.key, q);
            let count = g.tools.iter().filter(filter).count();
            if count == 0 {
                continue;
            }
            let (expanded, _) = group(ui, g.key, tr(g.key), &count.to_string(), icon, q);
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
            let filter = |t: &&logisim_core::plugin::api::Tool| plugin_matches(lib, t, q);
            let count = m.tools.iter().filter(filter).count();
            if count == 0 {
                continue;
            }
            let (expanded, header) = group(
                ui,
                (&m.id, &m.version),
                &name,
                &count.to_string(),
                Icon::Chip,
                q,
            );
            header.on_hover_ui(|ui| {
                ui.label(format!("{} · v{}", m.id, m.version));
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
            let ids: Vec<_> = self
                .project()
                .circuits
                .iter()
                .filter(|d| {
                    g.circuits.contains(&d.id) && (matches(&g.name, q) || matches(&d.name, q))
                })
                .map(|d| d.id)
                .collect();
            if ids.is_empty() {
                continue;
            }
            let (expanded, header) = group(
                ui,
                ("imported", g.circuits[0]),
                &g.name,
                &ids.len().to_string(),
                Icon::Document,
                q,
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
                    theme::palette(ui.ctx()).warning,
                );
                response.clone().on_hover_text(tr("Missing plugin"));
            }
        }
        let can_place = self.project().can_instantiate(self.active_circuit(), id);
        let response = response.on_hover_text(tr(if can_place {
            "Double-click to edit; single-click another circuit to place an instance."
        } else {
            "Double-click to edit this circuit."
        }));
        if response.double_clicked() {
            self.editor.switch_circuit(id);
        } else if response.clicked() && can_place {
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
                self.editor.session.context.appearance_mode = false;
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
