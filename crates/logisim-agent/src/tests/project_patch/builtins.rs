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
        ("Subtractor", json!({"bits":13})),
        ("Multiplier", json!({"bits":19})),
        ("Divider", json!({"bits":9})),
        ("Negator", json!({"bits":32})),
        ("BitAdder", json!({"bits":4,"bit_inputs":17})),
        ("BitFinder", json!({"bits":18,"search":"highest_zero"})),
        (
            "Priority Encoder",
            json!({"select_bits":5,"disabled_zero":true,"no_input_zero":true}),
        ),
        (
            "Bit Selector",
            json!({"bits":24,"group_bits":4,"top_right":true}),
        ),
        ("7-Segment Display Decoder", json!({"multibit":false})),
        (
            "Controlled Buffer",
            json!({"bits":6,"control_left":true,"negate_control":true,"label":"drive"}),
        ),
        (
            "Controlled Inverter",
            json!({"bits":3,"size":"wide","control_left":true}),
        ),
        (
            "Transistor",
            json!({"bits":4,"transistor_type":"n","gate_bottom_right":true}),
        ),
        (
            "Transmission Gate",
            json!({"bits":12,"gate_bottom_right":true}),
        ),
        (
            "D Flip-Flop",
            json!({"trigger":"falling","controls":"below_above","negate_controls":true}),
        ),
        (
            "T Flip-Flop",
            json!({"new_layout":false,"controls":"legacy"}),
        ),
        (
            "JK Flip-Flop",
            json!({"trigger":"falling","label":"Q","controls":"legacy"}),
        ),
        (
            "SR Flip-Flop",
            json!({"controls":"below_above","negate_controls":true}),
        ),
        (
            "D Latch",
            json!({"trigger":"low","new_layout":false,"controls":"above_below"}),
        ),
        (
            "SR Latch",
            json!({"trigger":"latch","new_layout":false,"controls":"below_above","negate_controls":true}),
        ),
        (
            "Register",
            json!({"bits":17,"select_high":true,"trigger":"low"}),
        ),
        (
            "Counter",
            json!({"bits":32,"counter_max":4294967295u32,"counter_overflow":"load","counter_old":true,"trigger":"falling"}),
        ),
        (
            "ShiftRegister",
            json!({"bits":31,"trigger":"falling","font":{"family":"Serif","size":19,"italic":true,"bold":false}}),
        ),
        (
            "Random",
            json!({"bits":12,"random_seed":-2147483648i32,"trigger":"falling","color":[0,120,241,255]}),
        ),
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
            let hint = match v {
                Value::Bool(_) => "boolean",
                Value::String(_) => "string",
                Value::Number(_) => "integer",
                Value::Object(_) => "object",
                Value::Array(_) => "array",
                Value::Null => panic!("unexpected null fixture"),
            };
            assert_eq!(c["property_types"][key], hint, "{kind}.{key}");
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
            "Bit Extender"
                | "Multiplexer"
                | "RGBLED"
                | "DotMatrix"
                | "Adder"
                | "BitAdder"
                | "Bit Selector"
                | "Priority Encoder"
                | "7-Segment Display Decoder"
                | "Controlled Inverter"
                | "Transmission Gate"
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
        ("D Flip-Flop", json!({"bits":2})),
        ("D Flip-Flop", json!({"controls":"Above / below"})),
        (
            "T Flip-Flop",
            json!({"new_layout":false,"controls":"below_above"}),
        ),
        ("SR Flip-Flop", json!({"trigger":"high"})),
        ("D Latch", json!({"trigger":"latch"})),
        ("SR Latch", json!({"trigger":"rising"})),
        ("Register", json!({"trigger":"level"})),
        ("Counter", json!({"bits":4,"counter_max":16})),
        ("Counter", json!({"trigger":"low"})),
        ("Counter", json!({"counter_overflow":"roll"})),
        ("ShiftRegister", json!({"bits":33})),
        ("Random", json!({"random_seed":2147483648i64})),
        ("Random", json!({"state":4})),
        ("BitAdder", json!({"bit_inputs":0})),
        ("BitFinder", json!({"search":"first"})),
        ("Priority Encoder", json!({"enable":true})),
        ("Bit Selector", json!({"group_bits":33})),
        ("7-Segment Display Decoder", json!({"bits":8})),
        ("Controlled Buffer", json!({"size":"wide"})),
        ("Controlled Inverter", json!({"size":"medium"})),
        ("Controlled Inverter", json!({"pull":"1"})),
        ("Transistor", json!({"transistor_type":"npn"})),
        ("Transmission Gate", json!({"transistor_type":"n"})),
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

#[test]
fn add_and_update_reject_wrong_json_types_for_every_advertised_builtin_key() {
    let mut f = Fixture::new(SOURCE);
    let cases = cases();
    let adds: Vec<_> = cases.iter().enumerate().map(|(i, (kind, props))|
        json!({"op":"add_component","circuit":1,"kind":kind,"position":[100*i,200],"properties":props})).collect();
    f.patch(json!(adds), false).unwrap();
    let before = f.snapshot();
    for c in &before["circuits"][0]["components"].as_array().unwrap()[2..] {
        let keys = c["patchable_properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .chain(["unknown_field"]);
        for key in keys {
            let invalid = json!({(key):null});
            for op in [
                json!({"op":"add_component","circuit":1,"kind":c["kind"],"position":[40,40],"properties":invalid}),
                json!({"op":"update_component","circuit":1,"id":c["id"],"properties":invalid}),
            ] {
                let batch = json!([{"op":"rename_circuit","id":1,"name":"temporary"},op]);
                let error = f.patch(batch, false).unwrap_err();
                assert_eq!(error.code, "invalid_property", "{}.{}", c["kind"], key);
                assert_eq!(error.details["op_index"], 1);
            }
        }
    }
    assert_eq!(f.snapshot(), before);
}

#[test]
fn increasing_mux_inputs_preserves_the_wired_input_role_on_reopen() {
    use logisim_core::Point;
    let mut f = Fixture::new(SOURCE);
    let added = f
        .patch(
            json!([
                {"op":"add_component","kind":"Multiplexer","circuit":1,"position":[200,200]},
                {"op":"add_wire","circuit":1,"from":[100,190],"to":[170,190]}
            ]),
            false,
        )
        .unwrap();
    let id = &added["changes"][0]["id"];
    f.patch(json!([{"op":"update_component","circuit":1,"id":id,"properties":{"select_bits":3,"bits":7}}]),false).unwrap();
    f.call("project_save", json!({})).unwrap();
    let xml = fs::read_to_string(&f.path).unwrap();
    let loaded = logisim_file::parse(&xml).unwrap();
    assert!(loaded.warnings.is_empty());
    let circuit = &loaded.document.main_circuit().circuit;
    let mux = &circuit.components[2];
    let input = Point::new(160, 160);
    let idx = mux.ports().iter().position(|p| *p == input).unwrap();
    assert_eq!(mux.port_width(idx), Some(7));
    let start = Point::new(100, 190);
    let first = circuit
        .wires
        .iter()
        .find(|w| w.a == start || w.b == start)
        .unwrap();
    let net = circuit.connected_wires(first.id);
    assert!(
        circuit
            .wires
            .iter()
            .any(|w| net.contains(&w.id) && (w.a == input || w.b == input))
    );
    assert!(circuit.wires.iter().any(|w| w.contains(Point::new(10, 10))
        && w.contains(Point::new(70, 10))
        && !net.contains(&w.id)));
}
