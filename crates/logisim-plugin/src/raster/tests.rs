use super::*;
fn module(version: u32, call: &str, bytes: &[u8]) -> Vec<u8> {
    let manifest = Manifest {
        abi: 3,
        id: "raster.test".into(),
        version: "1".into(),
        name: "Raster".into(),
        tools: vec![logisim_plugin_api::Tool {
            id: "screen".into(),
            name: "Screen".into(),
            bounds: [[0, 0], [10, 10]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![],
        }],
    };
    let json = serde_json::to_vec(&Response::Manifest(manifest)).unwrap();
    let encoded = |b: &[u8]| b.iter().map(|b| format!("\\{b:02x}")).collect::<String>();
    wat::parse_str(format!(
        r#"(module (memory (export "memory") 1)
        (data (i32.const 4096) "{}") (data (i32.const 8192) "{}")
        (func (export "logisim_abi") (result i32) i32.const 3)
        (func (export "logisim_buffer") (param i32) (result i32) i32.const 0)
        (func (export "logisim_call") (param i32) (result i64) i64.const {})
        (func (export "logisim_raster_version") (result i32) i32.const {version})
        (func (export "logisim_raster_config") (param i32) (result i32) i32.const 0)
        {call})"#,
        encoded(&json),
        encoded(bytes),
        ((json.len() as u64) << 32) | 4096
    ))
    .unwrap()
}
#[test]
fn negotiation_memory_bounds_revision_and_geometry_are_checked() {
    let mut bytes = vec![];
    let mut image = raster::Image {
        revision: 1,
        width: 1,
        height: 1,
        bounds: [[0, 0], [10, 10]],
        pixels: vec![1, 2, 3, 255],
    };
    raster::write_output(&mut bytes, &raster::Output::Image(image.clone())).unwrap();
    let export = |packed: u64| {
        format!(
            r#"(func (export "logisim_raster_call") (param i32) (result i64) i64.const {packed})"#
        )
    };
    let packed = ((bytes.len() as u64) << 32) | 8192;
    let plugin = Plugin::load(&module(1, &export(packed), &bytes)).unwrap();
    let c = plugin.manifest().components().remove(0);
    let mut vm = plugin.instantiate(&c).unwrap();
    let first = vm.raster(&c, &[], None).unwrap().unwrap();
    assert_eq!(*first, image);
    assert!(Arc::ptr_eq(
        &first,
        &vm.raster(&c, &[], Some(&first)).unwrap().unwrap()
    ));
    let mut old = image.clone();
    old.pixels[0] = 9;
    assert!(vm.raster(&c, &[], Some(&Arc::new(old))).is_err());
    let unknown = Plugin::load(&module(99, "", &[])).unwrap();
    assert!(
        unknown
            .instantiate(&c)
            .unwrap()
            .raster(&c, &[], None)
            .unwrap()
            .is_none()
    );
    assert!(Plugin::load(&module(1, "", &[])).is_err());
    for offset in [
        ((raster::MAX_RESPONSE as u64 + 1) << 32) | 8192,
        ((bytes.len() as u64) << 32) | 65530,
    ] {
        assert!(
            Plugin::load(&module(1, &export(offset), &bytes))
                .unwrap()
                .instantiate(&c)
                .unwrap()
                .raster(&c, &[], None)
                .is_err()
        );
    }
    for call in [
        r#"(func (export "logisim_raster_call") (param i32) (result i64) unreachable)"#,
        r#"(func (export "logisim_raster_call") (param i32) (result i64) (loop $s br $s) i64.const 0)"#,
    ] {
        assert!(
            Plugin::load(&module(1, call, &bytes))
                .unwrap()
                .instantiate(&c)
                .unwrap()
                .raster(&c, &[], None)
                .is_err()
        );
    }
    image.bounds[1][0] = 11;
    raster::write_output(&mut bytes, &raster::Output::Image(image)).unwrap();
    assert!(
        Plugin::load(&module(1, &export(packed), &bytes))
            .unwrap()
            .instantiate(&c)
            .unwrap()
            .raster(&c, &[], None)
            .is_err()
    );
    raster::write_output(&mut bytes, &raster::Output::Unchanged(1)).unwrap();
    let plugin = Plugin::load(&module(
        1,
        &export(((bytes.len() as u64) << 32) | 8192),
        &bytes,
    ))
    .unwrap();
    let mut vm = plugin.instantiate(&c).unwrap();
    assert!(vm.raster(&c, &[], None).is_err());
    assert!(Arc::ptr_eq(
        &first,
        &vm.raster(&c, &[], Some(&first)).unwrap().unwrap()
    ));
}
