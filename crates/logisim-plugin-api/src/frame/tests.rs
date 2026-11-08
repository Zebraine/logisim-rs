use super::*;
#[test]
fn wire_layout_borrowed_state_and_all_graphic_kinds_round_trip() {
    assert_eq!(
        (size_of::<Header>(), size_of::<Port>(), size_of::<Graphic>()),
        (32, 16, 40)
    );
    let signals = [Signal {
        width: 32,
        value: 0x8000_0001,
        unknown: 2,
        error: 4,
    }];
    let state = [0, 1, 255, 0, 42];
    let mut bytes = vec![0; request_len(&signals, &state).unwrap()];
    write_request(&mut bytes, Operation::Transition, &signals, &state).unwrap();
    assert_eq!(&bytes[..12], b"LSF1\x01\0\0\0\x02\0\0\0");
    assert_eq!(&bytes[32..40], &[32, 0, 0, 0, 1, 0, 0, 128]);
    let view = Input::read(&bytes).unwrap();
    assert_eq!(view.signals(), signals);
    assert_eq!(view.state, state);
    assert_eq!(view.state.as_ptr(), bytes[48..].as_ptr());
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&bytes);
    assert_eq!(Input::read(&unaligned[1..]).unwrap().signals(), signals);
    let output = Output::Outputs(StableOutputs {
        outputs: signals.to_vec(),
        graphics: vec![
            Shape::Rect {
                min: [-10, -20],
                max: [10, 20],
                fill: [1, 2, 3, 4],
                stroke: [5, 6, 7, 8],
            },
            Shape::Line {
                a: [-4096, 0],
                b: [4096, 1],
                color: [0; 4],
            },
            Shape::Ellipse {
                min: [-1, -2],
                max: [1, 2],
                fill: [255; 4],
                stroke: [0; 4],
            },
            Shape::Text {
                at: [3, 4],
                text: "测试 Rust 🦀".into(),
                size: 12,
                color: [255; 4],
            },
            Shape::Text {
                at: [0, 0],
                text: "".into(),
                size: 6,
                color: [0; 4],
            },
        ],
    });
    write_output(&mut bytes, &output).unwrap();
    assert_eq!(read_output(&bytes, Operation::Outputs).unwrap(), output);
    for outputs_changed in [false, true] {
        for fixed_point in [false, true] {
            let output = Output::Transition(StateTransition {
                state: state.to_vec(),
                outputs_changed,
                fixed_point,
            });
            write_output(&mut bytes, &output).unwrap();
            assert_eq!(read_output(&bytes, Operation::Transition).unwrap(), output);
        }
    }
}
#[test]
fn hostile_headers_lengths_signals_graphics_and_errors_are_bounded() {
    let signal = Signal {
        width: 1,
        value: 1,
        unknown: 0,
        error: 0,
    };
    let mut request = vec![0; request_len(&[signal], &[42]).unwrap()];
    write_request(&mut request, Operation::Outputs, &[signal], &[42]).unwrap();
    for len in 0..request.len() {
        assert!(Input::read(&request[..len]).is_err());
    }
    let mut bad = request.clone();
    bad.push(0);
    assert!(Input::read(&bad).is_err());
    for (offset, value) in [
        (0, 0),
        (4, 2),
        (8, 3),
        (12, 1),
        (16, 65),
        (20, u32::MAX),
        (24, 1),
        (28, 1),
        (32, 257),
        (40, 1),
    ] {
        let mut bad = request.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(Input::read(&bad).is_err(), "{offset}");
    }
    assert!(request_len(&vec![signal; 65], &[]).is_err());
    assert!(request_len(&[], &vec![0; MAX_STATE + 1]).is_err());
    let output = Output::Outputs(StableOutputs {
        outputs: vec![],
        graphics: vec![Shape::Text {
            at: [0, 0],
            text: "ok".into(),
            size: 12,
            color: [0; 4],
        }],
    });
    let mut bytes = vec![];
    write_output(&mut bytes, &output).unwrap();
    for (offset, value) in [
        (12, 4),
        (20, 1),
        (32, 9),
        (56, 1),
        (60, 999),
        (64, u32::MAX),
        (68, u32::MAX),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(read_output(&bad, Operation::Outputs).is_err(), "{offset}");
    }
    assert!(read_output(&bytes, Operation::Transition).is_err());
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() = 255;
    assert!(read_output(&bad, Operation::Outputs).is_err());
    write_error(&mut bytes, &"错".repeat(3000));
    let error = read_output(&bytes, Operation::Outputs).unwrap_err();
    assert_eq!(error.len(), 2046);
    assert!(bytes.len() <= 2080);
    // Arbitrary truncated/adversarial messages must return errors, never panic.
    let mut n = 42u32;
    for len in 0..512 {
        let bytes = (0..len)
            .map(|_| {
                n = n.wrapping_mul(1664525).wrapping_add(1013904223);
                (n >> 24) as u8
            })
            .collect::<Vec<_>>();
        assert!(Input::read(&bytes).is_err());
        assert!(read_output(&bytes, Operation::Outputs).is_err());
    }
}
