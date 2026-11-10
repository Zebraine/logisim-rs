use super::*;
fn values() -> Vec<Signal> {
    model::manifest().tools[0]
        .ports
        .iter()
        .map(|p| Signal {
            width: p.width,
            value: 0,
            unknown: 0,
            error: 0,
        })
        .collect()
}
#[test]
fn buffer_edges_clear_priority_unknowns_and_atomic_presentation() {
    let mut inputs = values();
    let mut s = State::new(16);
    s.sample(&inputs, 16).unwrap();
    inputs[1].value = 15;
    inputs[2].value = 3;
    inputs[3].value = 224;
    inputs[4].value = 1;
    inputs[0].value = 1;
    s.sample(&inputs, 16).unwrap();
    assert_eq!(s.back[63], 224);
    assert_eq!(s.front[63], 0);
    assert_eq!(s.revision, 0);
    inputs[6].value = 1;
    s.sample(&inputs, 16).unwrap();
    assert_eq!(s.revision, 0);
    inputs[0].value = 0;
    s.sample(&inputs, 16).unwrap();
    inputs[0].value = 1;
    s.sample(&inputs, 16).unwrap();
    assert_eq!(s.front[63], 224);
    assert_eq!(s.revision, 1);
    for field in [1, 2, 3, 4, 0] {
        inputs[0].value = 0;
        s.sample(&inputs, 16).unwrap();
        inputs[0].value = 1;
        inputs[3].value = 255;
        inputs[field].unknown = 1;
        inputs[field].value &= !1;
        s.sample(&inputs, 16).unwrap();
        assert_eq!(s.revision, 1);
        inputs[field].unknown = 0;
    }
    inputs = values();
    inputs[0].value = 0;
    s.sample(&inputs, 16).unwrap();
    inputs[0].value = 1;
    inputs[3].value = 3;
    inputs[4].value = 1;
    inputs[5].value = 1;
    inputs[6].value = 1;
    s.sample(&inputs, 16).unwrap();
    assert!(s.front.iter().all(|b| *b == 3));
    assert_eq!(s.revision, 2);
    assert_eq!(State::decode(&s.encode(), 16).unwrap().front, s.front);
    assert!(State::decode(&s.encode(), 32).is_err());
    s.revision = u64::MAX;
    inputs[0].value = 0;
    s.sample(&inputs, 16).unwrap();
    inputs[0].value = 1;
    inputs[3].value = 0;
    assert!(s.sample(&inputs, 16).is_err());
}
#[test]
fn native_transport_palette_reset_and_out_of_bounds() {
    let manifest = model::manifest();
    manifest.validate().unwrap();
    presentation().validate(manifest).unwrap();
    for c in manifest.components() {
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
        };
        let mut inputs = values();
        let mut state = State::new(model::tool(&c.tool.id).unwrap().1);
        state.clock = 0;
        inputs[0].value = 1;
        inputs[1].value = 31;
        inputs[2].value = 31;
        inputs[3].value = 255;
        inputs[4].value = 1;
        inputs[6].value = 1;
        let frame::Output::Transition(next) = stable_frame(
            &config,
            frame::Operation::Transition,
            &inputs,
            &state.encode(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(next.outputs_changed, c.tool.id == "framebuffer32");
        assert!(next.fixed_point);
        let raster::Output::Image(image) = raster(&config, &next.state, None).unwrap() else {
            panic!()
        };
        assert_eq!(image.pixels.last(), Some(&255));
        assert!(image.validate_tool(&c.tool).is_ok());
        assert_eq!(
            raster(&config, &next.state, Some(image.revision)).unwrap(),
            raster::Output::Unchanged(image.revision)
        );
        let Response::Update(reset) = handle(Request::Evaluate(Evaluation {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
            inputs,
            state: next.state,
            event: Event::Reset,
        })) else {
            panic!()
        };
        let raster::Output::Image(image) = raster(&config, &reset.state, None).unwrap() else {
            panic!()
        };
        assert!(
            image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [0, 0, 0, 255])
        );
        let mut state = State::decode(&reset.state, image.width as usize).unwrap();
        state.front[0] = 128;
        let raster::Output::Image(rgb) = raster(&config, &state.encode(), None).unwrap() else {
            panic!()
        };
        assert_eq!(&rgb.pixels[..4], &[145, 0, 0, 255]);
        let mut grayscale = config;
        grayscale
            .properties
            .insert("palette".into(), "Grayscale".into());
        let raster::Output::Image(gray) = raster(&grayscale, &state.encode(), None).unwrap() else {
            panic!()
        };
        assert_eq!(&gray.pixels[..4], &[128, 128, 128, 255]);
    }
}
