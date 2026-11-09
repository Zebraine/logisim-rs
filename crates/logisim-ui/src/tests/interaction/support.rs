//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::*;
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};

pub(super) fn frame(
    ctx: &Context,
    app: &mut Workspace,
    mut events: Vec<Event>,
    modifiers: Modifiers,
) {
    events.insert(0, Event::ModifiersChanged(modifiers));
    let time = ctx.input(|i| i.time) + 0.2;
    let focused = events
        .iter()
        .filter_map(|event| match event {
            Event::WindowFocused(focused) => Some(*focused),
            _ => None,
        })
        .next_back()
        .unwrap_or(true);
    let mut out = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 820.0))),
            time: Some(time),
            focused,
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
}
pub(super) fn setup() -> (Context, Workspace) {
    let ctx = Context::default();
    configure(&ctx);
    let mut app = Workspace::default();
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
    }
    (ctx, app)
}
pub(super) fn button(ctx: &Context, app: &mut Workspace, pos: Pos2, pressed: bool) {
    button_with(ctx, app, pos, pressed, Modifiers::NONE);
}
pub(super) fn button_with(
    ctx: &Context,
    app: &mut Workspace,
    pos: Pos2,
    pressed: bool,
    modifiers: Modifiers,
) {
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers,
            },
        ],
        modifiers,
    );
}
pub(super) fn click(ctx: &Context, app: &mut Workspace, pos: Pos2) {
    button(ctx, app, pos, true);
    button(ctx, app, pos, false);
}
pub(super) fn key(ctx: &Context, app: &mut Workspace, key: Key, modifiers: Modifiers) {
    frame(
        ctx,
        app,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        modifiers,
    );
    frame(
        ctx,
        app,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }],
        Modifiers::NONE,
    );
}
pub(super) fn drag(ctx: &Context, app: &mut Workspace, a: Pos2, b: Pos2) {
    button(ctx, app, a, true);
    frame(ctx, app, vec![Event::PointerMoved(b)], Modifiers::NONE);
    button(ctx, app, b, false);
}
