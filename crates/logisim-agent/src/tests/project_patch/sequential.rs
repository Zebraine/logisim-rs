use super::{Fixture, SOURCE, open};
use logisim_core::Point;
use serde_json::json;
use std::fs;

#[test]
fn counter_width_and_max_are_validated_together_without_overwriting_other_settings() {
    let mut f = Fixture::new(SOURCE);
    let added = f.patch(json!([{"op":"add_component","kind":"Counter","circuit":1,
        "position":[200,100],"properties":{"counter_max":240,"counter_overflow":"stay","label":"PC"}}]),false).unwrap();
    let id = &added["changes"][0]["id"];
    f.call("simulation_open", json!({})).unwrap();
    let before = f.snapshot();
    let runtime = f.call("simulation_state", json!({})).unwrap();
    let bad = json!([
        {"op":"update_component","circuit":1,"id":id,"properties":{"trigger":"falling"}},
        {"op":"update_component","circuit":1,"id":id,"properties":{"bits":3}}
    ]);
    let error = f.patch(bad, false).unwrap_err();
    assert_eq!(error.details["op_index"], 1);
    assert_eq!(f.snapshot(), before);
    assert_eq!(f.call("simulation_state", json!({})).unwrap(), runtime);
    let valid = json!([{"op":"update_component","circuit":1,"id":id,
        "properties":{"bits":3,"counter_max":6,"trigger":"falling"}}]);
    f.patch(valid.clone(), true).unwrap();
    assert_eq!(f.call("simulation_state", json!({})).unwrap(), runtime);
    f.patch(valid, false).unwrap();
    assert_eq!(
        f.call("simulation_state", json!({})).unwrap_err().code,
        "simulation_not_open"
    );
    f.call("project_save", json!({})).unwrap();
    f.id = open(&mut f.cli, &f.path);
    let snap = f.snapshot();
    let counter = &snap["circuits"][0]["components"][2];
    assert_eq!(counter["bits"], 3);
    assert_eq!(counter["counter_max"], 6);
    assert_eq!(counter["trigger"], "falling");
    assert_eq!(counter["counter_overflow"], "stay");
    assert_eq!(counter["label"], "PC");
}

#[test]
fn flip_flop_layout_edit_keeps_the_inverted_output_connected_on_reopen() {
    let mut f = Fixture::new(SOURCE);
    let added = f
        .patch(
            json!([
                {"op":"add_component","circuit":1,"kind":"D Flip-Flop","position":[200,200]},
                {"op":"add_wire","circuit":1,"from":[200,240],"to":[300,240]}
            ]),
            false,
        )
        .unwrap();
    let id = &added["changes"][0]["id"];
    f.patch(
        json!([{"op":"update_component","circuit":1,"id":id,
        "properties":{"new_layout":false,"controls":"legacy"}}]),
        false,
    )
    .unwrap();
    f.call("project_save", json!({})).unwrap();
    let imported = logisim_file::parse(&fs::read_to_string(&f.path).unwrap()).unwrap();
    assert!(imported.warnings.is_empty());
    let circuit = &imported.document.main_circuit().circuit;
    let q_bar = circuit.components[2].ports()[1];
    assert_eq!(q_bar, Point::new(200, 220));
    let far_end = Point::new(300, 240);
    let first = circuit
        .wires
        .iter()
        .find(|w| w.a == far_end || w.b == far_end)
        .unwrap();
    let net = circuit.connected_wires(first.id);
    assert!(
        circuit
            .wires
            .iter()
            .any(|w| net.contains(&w.id) && (w.a == q_bar || w.b == q_bar))
    );
}
