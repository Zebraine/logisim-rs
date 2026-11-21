use super::{Fixture, SOURCE, open};
use serde_json::json;

#[test]
fn io_edits_move_and_reopen_with_only_persistent_properties() {
    let mut f = Fixture::new(SOURCE);
    let cases = [
        (
            "Slider",
            json!({"bits":32,"slider_reverse":true,"slider_value":u32::MAX,"label":"level","font":{"family":"Serif","size":20,"italic":true,"bold":true},"color":[10,20,30,255]}),
        ),
        ("Joystick", json!({"bits":5,"facing":"West"})),
        (
            "Keyboard",
            json!({"bits":16,"trigger":"falling","buffer_len":256}),
        ),
        (
            "TTY",
            json!({"bits":16,"trigger":"falling","rows":48,"columns":120,"on_color":[1,2,3,4],"background":[9,8,7,6]}),
        ),
    ];
    let adds = cases
        .iter()
        .enumerate()
        .map(
            |(i, (k, _))| json!({"op":"add_component","circuit":1,"kind":k,"position":[300*i,300]}),
        )
        .collect::<Vec<_>>();
    let added = f.patch(json!(adds), false).unwrap();
    for (i, (_, props)) in cases.iter().enumerate() {
        let id = &added["changes"][i]["id"];
        f.patch(
            json!([{"op":"update_component","id":id,"properties":props},
            {"op":"move_component","id":id,"position":[300*i+40,450]}]),
            false,
        )
        .unwrap();
    }
    f.call("project_save", json!({})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    let snap = f.snapshot();
    let rows = snap["circuits"][0]["components"].as_array().unwrap();
    for (i, (kind, props)) in cases.iter().enumerate() {
        let c = &rows[2 + i];
        assert_eq!(c["kind"], *kind);
        assert_eq!(c["position"], json!([300 * i + 40, 450]));
        for (k, v) in props.as_object().unwrap() {
            assert_eq!(&c[k], v);
        }
        if *kind != "Slider" {
            for key in ["font", "label", "offset", "cursor", "screen", "buffer"] {
                assert!(c.get(key).is_none() && c["property_schema"].get(key).is_none());
            }
        }
    }
    for c in &rows[2..] {
        f.patch(json!([{"op":"remove_component","id":c["id"]}]), false)
            .unwrap();
    }
    f.call("project_save", json!({})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    assert_eq!(
        f.snapshot()["circuits"][0]["components"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn io_widths_character_encodings_and_runtime_fields_reject_invalid_edits() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    for (kind, props) in [
        ("Slider", json!({"bits":2,"slider_value":4})),
        ("Joystick", json!({"bits":1})),
        ("Joystick", json!({"bits":6})),
        ("Joystick", json!({"font":{"size":10}})),
        ("Joystick", json!({"offset":[0,0]})),
        ("Keyboard", json!({"bits":8})),
        ("Keyboard", json!({"buffer_len":257})),
        ("Keyboard", json!({"buffer":"x"})),
        ("TTY", json!({"rows":49})),
        ("TTY", json!({"columns":0})),
        ("TTY", json!({"trigger":"high"})),
        ("TTY", json!({"screen":"x"})),
    ] {
        let err=f.patch(json!([{"op":"add_component","kind":kind,"circuit":1,"position":[100,300],"properties":props}]),false).unwrap_err();
        assert_eq!(err.details["op_index"], 0);
    }
    assert_eq!(f.snapshot(), before);
}
