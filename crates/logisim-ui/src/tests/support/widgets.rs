use crate::*;
use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};

pub(crate) fn frame(ctx: &Context, app: &mut Workspace, events: Vec<Event>) -> Vec<(String, Rect)> {
    frame_at_size(ctx, app, events, egui::vec2(1200.0, 820.0))
}
pub(crate) fn frame_at_size(
    ctx: &Context,
    app: &mut Workspace,
    events: Vec<Event>,
    size: egui::Vec2,
) -> Vec<(String, Rect)> {
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    fn collect(shape: Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            Shape::Text(t) => out.push((
                t.galley.job.text.clone(),
                Rect::from_min_size(t.pos, t.galley.size()),
            )),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, out);
                }
            }
            _ => {}
        }
    }
    output.textures_delta.clear();
    let mut out = vec![];
    for shape in output.shapes {
        collect(shape.shape, &mut out);
    }
    out
}
pub(crate) fn click(ctx: &Context, app: &mut Workspace, text: &str) {
    let mut labels = frame(ctx, app, vec![]);
    for _ in 0..2 {
        labels = frame(ctx, app, vec![]);
    }
    let pos = labels
        .iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("Missing {text}: {labels:?}"))
        .1
        .center();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}
pub(crate) fn has(labels: &[(String, Rect)], expected: &str) -> bool {
    labels.iter().any(|(text, _)| text == expected)
}
