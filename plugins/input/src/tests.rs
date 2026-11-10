use super::*;
use interaction::{InputEvent, Key};
fn config() -> Properties {
    model::manifest().components().remove(0).properties
}
fn inputs(clock: u8, read: u8, clear: u8) -> Vec<Signal> {
    tool()
        .ports
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let v = [clock, read, clear].get(i).copied().unwrap_or(0);
            Signal {
                width: p.width,
                value: u32::from(v == 1),
                unknown: u32::from(v == 2),
                error: u32::from(v == 3),
            }
        })
        .collect()
}
#[test]
fn mapped_transitions_are_ordered_bounded_and_hold_state_even_after_overflow() {
    let p = config();
    let mut s = State::default();
    for (key, pressed) in [
        (Key::ArrowUp, true),
        (Key::ArrowUp, true),
        (Key::ArrowLeft, true),
        (Key::ArrowUp, false),
        (Key::ArrowLeft, false),
    ] {
        s.input(&p, 2, InputEvent::Key { key, pressed });
    }
    assert_eq!(s.mask(&p), 0);
    assert_eq!(s.queue.iter().copied().collect::<Vec<_>>(), [8, 10]);
    assert!(s.overflow);
    s.input(&p, 2, InputEvent::ReleaseAll {});
    assert!(s.queue.is_empty());
    assert!(!s.overflow);
    s.input(
        &p,
        2,
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: true,
        },
    );
    assert_eq!(s.mask(&p), 1);
}
#[test]
fn configurable_mapping_and_alias_channels_do_not_depend_on_game_controls() {
    let mut p = config();
    p.insert("key0".into(), "Q".into());
    p.insert("key7".into(), "Q".into());
    let mut s = State::default();
    s.input(
        &p,
        64,
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: true,
        },
    );
    assert_eq!(s.mask(&p), 0);
    assert!(s.queue.is_empty());
    s.input(
        &p,
        64,
        InputEvent::Key {
            key: Key::Q,
            pressed: true,
        },
    );
    assert_eq!(s.mask(&p), 129);
    assert_eq!(s.queue.iter().copied().collect::<Vec<_>>(), [8, 15]);
    s.input(
        &p,
        64,
        InputEvent::Key {
            key: Key::Q,
            pressed: false,
        },
    );
    assert_eq!(s.queue.iter().copied().collect::<Vec<_>>(), [8, 15, 0, 7]);
}
#[test]
fn only_known_selected_clock_edges_and_asserted_read_dequeue() {
    for falling in [false, true] {
        let mut p = config();
        if falling {
            p.insert("trigger".into(), "falling".into());
        }
        for prev in 0..4 {
            for next in 0..4 {
                for read in 0..4 {
                    let mut s = State {
                        clock: prev,
                        ..Default::default()
                    };
                    s.input(
                        &p,
                        16,
                        InputEvent::Key {
                            key: Key::ArrowUp,
                            pressed: true,
                        },
                    );
                    s.sample(&p, &inputs(next, read, 0));
                    let consumed =
                        read == 1 && (prev, next) == if falling { (1, 0) } else { (0, 1) };
                    assert_eq!(
                        s.queue.len(),
                        usize::from(!consumed),
                        "{falling} {prev}->{next} read={read}"
                    );
                    let saved = s.clone();
                    s.sample(&p, &inputs(next, read, 0));
                    assert_eq!(s, saved);
                }
            }
        }
    }
}
#[test]
fn clear_has_priority_and_unknown_controls_do_not_clear() {
    let p = config();
    for clear in 0..4 {
        let mut s = State::default();
        s.input(
            &p,
            16,
            InputEvent::Key {
                key: Key::ArrowUp,
                pressed: true,
            },
        );
        s.sample(&p, &inputs(0, 0, clear));
        assert_eq!(s.mask(&p), u32::from(clear != 1));
        assert_eq!(s.queue.len(), usize::from(clear != 1));
    }
}
#[test]
fn explicit_state_roundtrips_and_rejects_invalid_encodings() {
    let p = config();
    let mut s = State::default();
    for _ in 0..32 {
        for pressed in [true, false] {
            s.input(
                &p,
                64,
                InputEvent::Key {
                    key: Key::Enter,
                    pressed,
                },
            );
        }
    }
    let bytes = s.encode();
    assert_eq!(bytes.len(), 75);
    assert_eq!(State::decode(&bytes, 64).unwrap(), s);
    assert!(State::decode(&bytes, 63).is_err());
    for at in [0, 4, 9, 10, 11] {
        let mut bad = bytes.clone();
        bad[at] = 255;
        assert!(State::decode(&bad, 64).is_err());
    }
    assert!(State::decode(&bytes[..74], 64).is_err());
}
#[test]
fn describe_and_metadata_validate_and_invalid_actions_fail() {
    let manifest = model::manifest();
    manifest.validate().unwrap();
    interaction().validate(manifest).unwrap();
    presentation().validate(manifest).unwrap();
    for argument in [
        "{}",
        r#"{"type":"key","key":"Escape","pressed":true}"#,
        r#"{"type":"release_all","extra":0}"#,
    ] {
        assert!(matches!(
            handle(Request::Evaluate(Evaluation {
                tool: "keyboard".into(),
                properties: config(),
                inputs: inputs(0, 0, 0),
                state: vec![],
                event: Event::Action {
                    id: "key".into(),
                    argument: argument.into()
                }
            })),
            Response::Error(_)
        ));
    }
}
