//! Centered dialogs shared by the workspace and its desktop host.
use crate::i18n::tr;
use egui::{Align, Context, Frame, Id, Layout, ScrollArea, Ui, Vec2, vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dismiss {
    Open,
    Escape,
    Close,
}

pub struct Dialog {
    id: Id,
    size: Option<Vec2>,
}

impl Dialog {
    /// A content-sized prompt. Long messages scroll within the viewport.
    pub fn prompt(id: &'static str) -> Self {
        Self {
            id: Id::unique(id),
            size: None,
        }
    }

    /// A resizable editor with an independent, language-stable size.
    pub fn editor(id: &'static str, width: f32, height: f32) -> Self {
        Self {
            id: Id::unique(id),
            size: Some(vec2(width, height)),
        }
    }

    pub fn standard(id: &'static str, ctx: &Context) -> Self {
        let size = crate::theme::metrics(ctx).dialog;
        Self::editor(id, size.x, size.y)
    }

    pub fn large(id: &'static str, ctx: &Context) -> Self {
        let size = crate::theme::metrics(ctx).large_dialog;
        Self::editor(id, size.x, size.y)
    }

    pub fn show(
        self,
        ctx: &Context,
        title: &str,
        contents: impl FnOnce(&mut DialogUi<'_>),
    ) -> Dismiss {
        let frame = Frame::window(&ctx.global_style())
            .inner_margin(crate::theme::metrics(ctx).dialog_margin);
        let margin = frame.total_margin().sum();
        let limit = (ctx.content_rect().size() * 0.9 - margin).max(Vec2::splat(1.));
        let modal = egui::Modal::new(self.id).frame(frame).show(ctx, |ui| {
            let render = |ui: &mut Ui| {
                ui.add(egui::Label::new(egui::RichText::new(title).heading()).truncate())
                    .on_hover_text(title);
                ui.separator();
                contents(&mut DialogUi {
                    ui,
                    editor: self.size.is_some(),
                });
            };
            if let Some(size) = self.size {
                let min = (vec2(640., 440.).min(size) - margin).min(limit);
                egui::Resize::default()
                    .id_salt(("dialog-size", size.x.to_bits(), size.y.to_bits()))
                    .with_stroke(false)
                    .min_size(min)
                    .max_size(limit)
                    .default_size((size - margin).min(limit))
                    .show(ui, |ui| {
                        let size = ui.available_size().clamp(min, limit);
                        ui.set_min_size(size);
                        ui.set_max_size(size);
                        render(ui);
                    });
            } else {
                ui.set_width(400_f32.min(limit.x));
                ui.set_max_height(limit.y);
                render(ui);
            }
        });
        if modal.backdrop_response.clicked() || modal.response.should_close() {
            Dismiss::Close
        } else if modal.should_close() {
            Dismiss::Escape
        } else {
            Dismiss::Open
        }
    }

    /// Run the action only on an explicit confirmation.
    pub fn confirm(
        self,
        ctx: &Context,
        title: &str,
        message: &str,
        action: &str,
        accept: impl FnOnce(),
    ) -> Dismiss {
        let mut confirmed = false;
        let dismiss = self.show(ctx, title, |dialog| {
            dialog.body(|ui| ui.label(message));
            dialog.footer("Cancel", |ui| {
                confirmed = ui.button(action).clicked();
            });
        });
        if confirmed {
            accept();
            Dismiss::Close
        } else {
            dismiss
        }
    }

    pub fn message(self, ctx: &Context, title: &str, message: &str) -> Dismiss {
        self.show(ctx, title, |dialog| {
            dialog.body(|ui| ui.label(message));
            dialog.footer("OK", |_| {});
        })
    }
}

pub struct DialogUi<'a> {
    ui: &'a mut Ui,
    editor: bool,
}

impl DialogUi<'_> {
    pub fn body_height(&self) -> f32 {
        let spacing = self.ui.spacing();
        (self.ui.available_height() - spacing.interact_size.y - 4. * spacing.item_spacing.y).max(1.)
    }

    pub fn body<R>(&mut self, contents: impl FnOnce(&mut Ui) -> R) -> R {
        ScrollArea::vertical()
            .id_salt("dialog-content")
            .max_height(self.body_height())
            .min_scrolled_height(1.)
            .auto_shrink([false, !self.editor])
            .show(self.ui, contents)
            .inner
    }

    /// A bounded region for an editor that manages its own scroll panes.
    pub(crate) fn panes(&mut self, contents: impl FnOnce(&mut Ui)) {
        let size = vec2(self.ui.available_width(), self.body_height());
        self.ui
            .allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
                ui.set_height(size.y);
                contents(ui);
            });
    }

    /// The dismiss button sits at the trailing edge. Additional actions follow inward.
    pub fn footer(&mut self, dismiss: &'static str, actions: impl FnOnce(&mut Ui)) {
        self.actions(|ui| {
            if ui.button(tr(dismiss)).clicked() {
                ui.close();
            }
            actions(ui);
        });
    }
    /// A prompt that requires an explicit decision supplies both of its actions.
    pub fn actions(&mut self, actions: impl FnOnce(&mut Ui)) {
        self.ui.separator();
        self.ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.editor {
                    ui.add_space(ui.visuals().resize_corner_size);
                }
                actions(ui);
            });
        });
    }
}

#[cfg(test)]
mod tests;
