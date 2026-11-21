use super::{Fixture, SOURCE, open};
use serde_json::{Value, json};

fn add(kind: &str, properties: Value) -> Value {
    json!({"op":"add_component","circuit":1,"kind":kind,"position":[180,80],"properties":properties})
}

#[test]
fn rom_boundaries_and_ram_configuration_round_trip_with_final_dimensions() {
    let mut f = Fixture::new(SOURCE);
    let runs = json!([[0, 1], [5, 0], [16, 255], [16777215, u32::MAX]]);
    let changes = f
        .patch(
            json!([
                add(
                    "ROM",
                    json!({"address_bits":24,"bits":32,"contents":runs,"select_high":true})
                ),
                add(
                    "RAM",
                    json!({"address_bits":10,"bits":16,"interface":"shared_async","simple":false})
                )
            ]),
            false,
        )
        .unwrap();
    let id = &changes["changes"][0]["id"];
    let ram = &changes["changes"][1]["id"];
    let before = f.snapshot();
    assert_eq!(before["circuits"][0]["components"][2]["contents"], runs);
    for mode in ["shared_sync", "shared_async", "separate"] {
        f.patch(json!([{"op":"update_component","id":ram,"properties":{"interface":mode,"simple":false,"select_high":true}}]), false).unwrap();
        f.call("project_save", json!({})).unwrap();
        f.id = open(&mut f.cli, &f.path);
        let snap = f.snapshot();
        let c = &snap["circuits"][0]["components"][3];
        assert_eq!(c["interface"], mode);
        assert_eq!(c["bits"], 16);
        assert_eq!(c["address_bits"], 10);
        assert_eq!(c["select_high"], true);
        assert_eq!(c["simple"], false);
        assert!(c.get("contents").is_none() && c["property_schema"].get("contents").is_none());
        assert_eq!(snap["circuits"][0]["components"][2]["contents"], runs);
    }
    for (props, expected) in [
        (
            json!({"address_bits":5,"bits":4}),
            json!([[0, 1], [5, 0], [16, 15]]),
        ),
        (
            json!({"address_bits":6}),
            json!([[0, 1], [5, 0], [16, 15], [32, 0]]),
        ),
        (json!({"contents":[]}), json!([])),
    ] {
        f.patch(
            json!([{"op":"update_component","id":id,"properties":props}]),
            false,
        )
        .unwrap();
        f.call("project_save", json!({})).unwrap();
        f.id = open(&mut f.cli, &f.path);
        assert_eq!(
            f.snapshot()["circuits"][0]["components"][2]["contents"],
            expected
        );
    }
}

#[test]
fn invalid_rom_images_and_ram_contents_roll_back_the_batch() {
    let mut f = Fixture::new(SOURCE);
    let before = f.snapshot();
    for bad in [
        json!([[1, 1], [0, 2]]),
        json!([[1, 1], [1, 2]]),
        json!([[4, 1]]),
        json!([[0, 4]]),
        json!([[-1, 0]]),
        json!([[0, -1]]),
        json!([[0, true]]),
        json!([[1]]),
        json!([[1, 2, 3]]),
        json!(vec![[0, 0]; 65537]),
    ] {
        let error = f
            .patch(
                json!([
                    add("RAM", json!({})),
                    add("ROM", json!({"address_bits":2,"bits":2,"contents":bad}))
                ]),
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, "invalid_property");
        assert_eq!(error.details["op_index"], 1);
    }
    for (kind, props) in [
        ("RAM", json!({"contents":[]})),
        ("ROM", json!({"interface":"separate"})),
        ("RAM", json!({"address_bits":25})),
        ("ROM", json!({"address_bits":1})),
    ] {
        assert!(f.patch(json!([add(kind, props)]), false).is_err());
    }
    assert_eq!(f.snapshot(), before);
}
