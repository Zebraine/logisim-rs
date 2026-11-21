use super::{Fixture, SOURCE, open};
use serde_json::json;

#[test]
fn pla_resizes_before_writing_planes_and_preserves_overlap_on_reopen() {
    let mut f = Fixture::new(SOURCE);
    let inputs = json!([
        ["direct", "inverted", "disconnected"],
        ["inverted", "direct", "direct"]
    ]);
    let outputs = json!([[true], [false]]);
    let added=f.patch(json!([{"op":"add_component","circuit":1,"kind":"PLA ROM","position":[180,80],
        "properties":{"inputs":3,"terms":2,"outputs":1,"input_plane":inputs,"output_plane":outputs,"select_high":true}}]),false).unwrap();
    let id = &added["changes"][0]["id"];
    for (props, expected_i, expected_o) in [
        (json!({}), inputs, outputs),
        (
            json!({"inputs":2,"terms":3,"outputs":2}),
            json!([
                ["direct", "inverted"],
                ["inverted", "direct"],
                ["disconnected", "disconnected"]
            ]),
            json!([[true, false], [false, false], [false, false]]),
        ),
        (
            json!({"terms":1,"outputs":3,"output_plane":[[true,false,true]]}),
            json!([["direct", "inverted"]]),
            json!([[true, false, true]]),
        ),
    ] {
        let updated = f
            .patch(
                json!([{"op":"update_component","id":id,"properties":props}]),
                false,
            )
            .unwrap();
        if props.get("terms").is_some() {
            let changed = &updated["changes"][0]["properties"];
            assert_eq!(changed["input_plane"], expected_i);
            assert_eq!(changed["output_plane"], expected_o);
        }
        f.call("project_save", json!({})).unwrap();
        f.id = open(&mut f.cli, &f.path);
        let snap = f.snapshot();
        let c = &snap["circuits"][0]["components"][2];
        assert_eq!(c["input_plane"], expected_i);
        assert_eq!(c["output_plane"], expected_o);
        assert_eq!(c["select_high"], true);
        assert_eq!(c["terms"], expected_i.as_array().unwrap().len());
        assert_eq!(c["inputs"], expected_i[0].as_array().unwrap().len());
        assert_eq!(c["outputs"], expected_o[0].as_array().unwrap().len());
    }
}

#[test]
fn invalid_pla_dimensions_shapes_and_cells_roll_back_without_mutating_other_planes() {
    let mut f = Fixture::new(SOURCE);
    let added = f
        .patch(
            json!([{"op":"add_component","kind":"PLA ROM","circuit":1,
        "position":[100,100],"properties":{"inputs":1,"terms":2,"outputs":1}}]),
            false,
        )
        .unwrap();
    let id = &added["changes"][0]["id"];
    let before = f.snapshot();
    for bad in [
        json!({"inputs":0}),
        json!({"outputs":33}),
        json!({"terms":-1}),
        json!({"input_plane":[]}),
        json!({"input_plane":[["direct"],[]]}),
        json!({"input_plane":[["direct"],["inverted"],["direct"]]}),
        json!({"input_plane":[["direct"],["none"]]}),
        json!({"output_plane":[[true],[1]]}),
        json!({"output_plane":[true,true]}),
        json!({"output_plane":[[],[true]]}),
    ] {
        let error = f
            .patch(
                json!([{"op":"update_component","id":id,"properties":{"select_high":true}},
            {"op":"update_component","id":id,"properties":bad}]),
                false,
            )
            .unwrap_err();
        assert_eq!(error.details["op_index"], 1);
        assert_eq!(error.code, "invalid_property");
        assert_eq!(f.snapshot(), before);
    }
}
