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

#[test]
fn workbench_dialog_footers_stay_in_view_in_each_density_and_language() {
    use crate::{
        Density, Language, Settings, Workspace,
        editor::Tool,
        i18n::{tr, with_language},
        tests::support::widgets::frame_at_size,
    };
    use logisim_core::Kind;
    for density in Density::ALL {
        for language in Language::ALL {
            for size in [vec2(720., 480.), vec2(1200., 820.)] {
                let ctx = Context::default();
                crate::configure(&ctx);
                for (id, footer) in [
                    ("circuit-analysis", "Close"),
                    ("circuit-generator", "Build Circuit"),
                    ("memory_contents_editor", "Next page"),
                    ("pla_program_editor", "Apply"),
                    ("project-options", "Close"),
                ] {
                    let mut w = Workspace::default();
                    w.apply_settings(Settings {
                        density,
                        language,
                        ..Default::default()
                    });
                    match id {
                        "circuit-analysis" => {
                            w.set_document(crate::DocumentView {
                                partial: true,
                                ..Default::default()
                            });
                            w.request_analysis();
                        }
                        "circuit-generator" => w.open_generator(),
                        "memory_contents_editor" => {
                            w.editor.set_tool(Tool::Place(Kind::Rom));
                            w.editor.memory_target = Some((true, w.editor.template.id));
                        }
                        "pla_program_editor" => {
                            w.editor.set_tool(Tool::Place(Kind::PlaRom));
                            w.editor.template.pla.resize(32, 24, 32);
                            w.editor.pla_target = Some((true, w.editor.template.id));
                        }
                        "project-options" => w.project_options_open = true,
                        _ => unreachable!(),
                    }
                    for _ in 0..3 {
                        frame_at_size(&ctx, &mut w, vec![], size);
                    }
                    let labels = frame_at_size(&ctx, &mut w, vec![], size);
                    let bounds = ctx.memory(|m| m.area_rect(Id::unique(id))).unwrap();
                    assert!(
                        bounds.width() <= size.x * 0.9 + 1. && bounds.height() <= size.y * 0.9 + 1.,
                        "{id} {bounds:?}"
                    );
                    let text = with_language(language, || tr(footer));
                    let button = labels.iter().rfind(|(s, _)| s == text).unwrap().1;
                    assert!(
                        bounds.contains_rect(button),
                        "{id} {density:?} {language:?}"
                    );
                    if id == "pla_program_editor" {
                        for (text, rect) in labels
                            .iter()
                            .filter(|(s, _)| s.starts_with('I') && s[1..].parse::<u8>().is_ok())
                        {
                            assert!(
                                rect.left() >= bounds.left() && rect.right() <= bounds.right(),
                                "{text} {rect:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
