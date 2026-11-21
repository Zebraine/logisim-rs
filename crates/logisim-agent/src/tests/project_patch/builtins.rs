use super::{Fixture, SOURCE, open};
use serde_json::{Value, json};
use std::fs;

fn cases() -> Vec<(&'static str, Value)> {
    vec![
        ("Constant", json!({"bits":32,"value":4294967295u32})),
        ("Ground", json!({"bits":4,"facing":"East"})),
        ("Power", json!({"bits":8,"facing":"West"})),
        ("Pull Resistor", json!({"pull":"X","facing":"North"})),
        (
            "Button",
            json!({"label":"B","label_facing":"center","color":[9,8,7,255],"on_color":[6,5,4,3]}),
        ),
        (
            "Switch",
            json!({"bits":7,"on_color":[1,2,3,4],"label_facing":"West"}),
        ),
        ("DipSwitch", json!({"switch_count":5,"label_facing":"East"})),
        (
            "Clock",
            json!({"high_ticks":65535,"low_ticks":3,"label_facing":"North"}),
        ),
        (
            "Text",
            json!({"text":"测量\nA & B","halign":"right","valign":"base","font":{"family":"Serif","bold":true,"italic":true,"size":21},"color":[7,8,9,63]}),
        ),
        (
            "Bit Extender",
            json!({"in_width":3,"out_width":21,"type":"input"}),
        ),
        ("Shifter", json!({"bits":12,"shift":"arithmetic_right"})),
        ("Adder", json!({"bits":23})),
        ("Comparator", json!({"bits":31,"signed":false})),
        (
            "Multiplexer",
            json!({"select_bits":3,"bits":9,"enable":true,"top_right":true,"disabled_zero":true}),
        ),
        (
            "Demultiplexer",
            json!({"select_bits":2,"bits":6,"three_state":true,"enable":true}),
        ),
        (
            "Decoder",
            json!({"select_bits":5,"three_state":true,"disabled_zero":true}),
        ),
        (
            "LED",
            json!({"on_color":[5,6,7,8],"off_color":[9,8,7,6],"active_high":false}),
        ),
        (
            "SevenSegment",
            json!({"active_high":false,"background":[20,30,40,50]}),
        ),
        (
            "HexDisplay",
            json!({"background":[2,3,4,5],"off_color":[1,3,5,7]}),
        ),
        ("RGBLED", json!({"multibit":true,"label_facing":"South"})),
        (
            "DotMatrix",
            json!({"matrix_rows":8,"matrix_columns":13,"matrix_input":"select","matrix_circular":true,"matrix_persistence":2147483647}),
        ),
    ]
}

#[test]
fn common_builtins_round_trip_typed_properties_ports_and_positions() {
    let mut f = Fixture::new(SOURCE);
    let cases = cases();
    let ops: Vec<_> = cases.iter().enumerate().map(|(i,(kind,_))|
        json!({"op":"add_component","circuit":1,"kind":kind,"position":[100*i,200]})).collect();
    let added = f.patch(json!(ops), false).unwrap();
    let initial = f.snapshot();
    let updates: Vec<_> = cases.iter().enumerate().map(|(i,(_,props))|
        json!({"op":"update_component","circuit":1,"id":added["changes"][i]["id"],"properties":props})).collect();
    f.patch(json!(updates), false).unwrap();
    let edited = f.snapshot();
    for (i, (kind, props)) in cases.iter().enumerate() {
        let c = &edited["circuits"][0]["components"][i + 2];
        for (key, v) in props.as_object().unwrap() {
            assert_eq!(&c[key], v, "{kind}.{key}");
            assert!(c["property_types"][key].is_string(), "{kind}.{key}");
            assert!(
                c["patchable_properties"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(key))
            );
        }
        assert_eq!(c["position"], json!([100 * i, 200]));
        if matches!(
            *kind,
            "Bit Extender" | "Multiplexer" | "RGBLED" | "DotMatrix" | "Adder"
        ) {
            assert_ne!(
                c["ports"],
                initial["circuits"][0]["components"][i + 2]["ports"],
                "{kind}"
            );
        }
    }
    f.call("project_save", json!({})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    let reopened = f.snapshot();
    assert_eq!(reopened["partial"], false);
    for (i, (kind, _)) in cases.iter().enumerate() {
        let c = &reopened["circuits"][0]["components"][i + 2];
        let before = &edited["circuits"][0]["components"][i + 2];
        for key in before["patchable_properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .chain(["ports", "position", "kind"])
        {
            assert_eq!(c[key], before[key], "{kind}.{key}");
        }
    }
    let removes: Vec<_> = reopened["circuits"][0]["components"].as_array().unwrap()[2..]
        .iter()
        .map(|c| json!({"op":"remove_component","circuit":1,"id":c["id"]}))
        .collect();
    f.patch(json!(removes), false).unwrap();
    assert_eq!(
        f.snapshot()["circuits"][0]["components"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn invalid_common_properties_roll_back_the_entire_batch_at_the_failing_index() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    for (kind, props) in [
        ("Clock", json!({"frequency":10})),
        ("Clock", json!({"high_ticks":65536})),
        ("Constant", json!({"bits":3,"value":8})),
        ("Ground", json!({"bits":0})),
        ("Button", json!({"on_color":[1,2,3]})),
        ("Switch", json!({"label_facing":"center"})),
        ("DipSwitch", json!({"switch_count":33})),
        ("Pull Resistor", json!({"pull":"up"})),
        ("Text", json!({"halign":"top"})),
        ("Text", json!({"valign":3})),
        ("Text", json!({"text":"x".repeat(65537)})),
        ("Text", json!({"font":{"family":""}})),
        ("LED", json!({"color":[1,2,3,0]})),
        ("Bit Extender", json!({"out_width":0})),
        ("Bit Extender", json!({"type":"unsigned"})),
        ("Shifter", json!({"shift":"leftish"})),
        ("Comparator", json!({"signed":1})),
        ("Adder", json!({"facing":"West"})),
        ("Multiplexer", json!({"select_bits":6})),
        ("Demultiplexer", json!({"three_state":1})),
        ("Decoder", json!({"bits":2})),
        ("RGBLED", json!({"multibit":"true"})),
        ("HexDisplay", json!({"bits":3})),
        ("DotMatrix", json!({"matrix_persistence":2147483648u32})),
        ("DotMatrix", json!({"matrix_input":"both"})),
    ] {
        let error = f.patch(json!([{"op":"add_circuit","name":"provisional"},
            {"op":"add_component","circuit":1,"position":[100,200],"kind":kind,"properties":props}]),false).unwrap_err();
        assert_eq!(error.details["op_index"], 1, "{kind}");
        assert_eq!(f.snapshot(), before, "{kind}");
    }
    assert_eq!(fs::read_to_string(&f.path).unwrap(), SOURCE);
}
