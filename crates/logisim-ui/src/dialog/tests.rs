use super::*;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2};

#[test]
fn long_bodies_scroll_above_the_footer_across_viewport_changes() {
    for editor in [false, true] {
        let ctx = Context::default();
        crate::configure(&ctx);
        for screen in [vec2(1440., 900.), vec2(480., 320.), vec2(1200., 820.)] {
            let mut body_clip = Rect::NOTHING;
            let mut button = Rect::NOTHING;
            for _ in 0..4 {
                let mut output = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
                        ..Default::default()
                    },
                    |ui| {
                        let window = if editor {
                            Dialog::editor("long-document", 760., 560.)
                        } else {
                            Dialog::prompt("long-document")
                        };
                        window.show(ui.ctx(), "A long document", |dialog| {
                            dialog.body(|ui| {
                                body_clip = ui.clip_rect();
                                ui.label("A long diagnostics report.\n".repeat(256));
                            });
                            dialog.footer("Cancel", |ui| button = ui.button("Confirm").rect);
                        });
                    },
                );
                output.textures_delta.clear();
            }
            let bounds = ctx.memory(|m| m.area_rect(Id::unique("long-document")).unwrap());
            assert!((bounds.center().to_vec2() - screen * 0.5).length() <= 1.);
            assert!(
                bounds.width() <= screen.x * 0.9 + 1. && bounds.height() <= screen.y * 0.9 + 1.
            );
            assert!(bounds.contains_rect(button));
            assert!(body_clip.bottom() <= button.top());
        }
    }
}

#[test]
fn escape_and_backdrop_never_confirm_a_destructive_prompt() {
    let ctx = Context::default();
    let mut accepted = false;
    let mut frame = |events| {
        let mut result = Dismiss::Open;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(720., 480.))),
                events,
                ..Default::default()
            },
            |ui| {
                result = Dialog::prompt("replace").confirm(
                    ui.ctx(),
                    "Replace?",
                    "File",
                    "Replace",
                    || accepted = true,
                );
            },
        );
        output.textures_delta.clear();
        result
    };
    for _ in 0..3 {
        frame(vec![]);
    }
    assert!(
        frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE
        }]) == Dismiss::Escape
    );
    let pos = pos2(6., 6.);
    let click = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(vec![Event::PointerMoved(pos), click(true)]);
    assert!(frame(vec![click(false)]) == Dismiss::Close);
    assert!(!accepted);
}
