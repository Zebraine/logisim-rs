use super::*;
use crate::tests::support::Temp;
use logisim_ui::{Language, i18n::with_language};

#[test]
fn reading_images_is_bounded_translated_and_never_writes_the_source() {
    let dir = Temp::new();
    let path = dir.path("memory.hex");
    let source = b"v2.0 raw\n12 2*ab";
    std::fs::write(&path, source).unwrap();
    assert_eq!(read(&path, 4, 8).unwrap().get(2), 0xab);
    assert_eq!(std::fs::read(&path).unwrap(), source);
    for language in Language::ALL {
        with_language(language, || {
            assert!(read(&dir.path("absent.hex"), 4, 8).is_err());
            assert_eq!(
                read(&dir.0, 4, 8).unwrap_err(),
                tr("Choose a regular .hex file.")
            );
            std::fs::write(&path, [0xff]).unwrap();
            assert_eq!(
                read(&path, 4, 8).unwrap_err(),
                tr("The memory image is not valid UTF-8 text.")
            );
            std::fs::write(&path, b"v2.0 raw\n12 invalid").unwrap();
            assert_eq!(
                read(&path, 4, 8).unwrap_err(),
                tf!("Line {}: invalid hexadecimal word.", 2)
            );
            let original = std::fs::read(&path).unwrap();
            assert!(read(&path, 4, 8).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
        });
    }
    std::fs::File::create(&path)
        .unwrap()
        .set_len(logisim_file::MAX_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(
        read(&path, 4, 8).unwrap_err(),
        "File exceeds the 8 MiB limit."
    );
}

#[test]
fn plugin_images_dispatch_once_only_after_file_and_target_validation() {
    use logisim_core::{
        Component, Point, Project,
        plugin::api::{self, presentation::*},
    };
    let manifest = api::Manifest {
        abi: api::ABI_VERSION,
        id: "test.memory".into(),
        version: "1".into(),
        name: "Test".into(),
        tools: vec![api::Tool {
            id: "memory".into(),
            name: "Memory".into(),
            bounds: [[0, 0], [20, 20]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![api::Action {
                id: "write".into(),
                label: "Write".into(),
                argument: Some("Words".into()),
            }],
        }],
    };
    let presentation = Presentation {
        imports: vec![MemoryImport {
            tool: "memory".into(),
            action: "write".into(),
            label: "Import".into(),
            address_bits: 5,
            data_bits: 32,
            argument_prefix: "image ".into(),
        }],
        ..Default::default()
    };
    manifest.validate().unwrap();
    presentation.validate(&manifest).unwrap();
    let mut project = Project::default();
    let id = project
        .main_circuit_mut()
        .circuit
        .add_component(Component::from_plugin(
            manifest.components().remove(0),
            Point::ZERO,
        ));
    let mut workspace = Workspace::default();
    workspace.replace_project(project.clone());
    workspace.set_plugin_catalogue(vec![Library {
        manifest,
        presentation,
    }]);
    let target = workspace.plugin_import_target(id, 0).unwrap();
    let dir = Temp::new();
    let path = dir.path("registers.hex");
    for bytes in [
        b"v2.0 raw\n12 badword".as_slice(),
        b"v2.0 raw\n33*1",
        b"bad header",
        &[0xff],
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert!(load_plugin(&path, &target, &mut workspace).is_err());
        assert!(workspace.take_simulation_actions().is_empty());
        assert_eq!(workspace.project(), &project);
    }
    assert!(load_plugin(&dir.path("absent.hex"), &target, &mut workspace).is_err());
    std::fs::write(&path, b"v2.0 raw\nffffffff 12345678 2*abcd").unwrap();
    load_plugin(&path, &target, &mut workspace).unwrap();
    let actions = workspace.take_simulation_actions();
    let [
        logisim_ui::SimulationAction::PluginAction {
            id: actual,
            action,
            argument,
        },
    ] = actions.as_slice()
    else {
        panic!("{actions:?}")
    };
    assert_eq!(*actual, id);
    assert_eq!(action, "write");
    let words: Vec<_> = argument
        .strip_prefix("image ")
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(words.len(), 32);
    assert_eq!(&words[..5], &["ffffffff", "12345678", "abcd", "abcd", "0"]);
    assert_eq!(workspace.project(), &project);
    workspace.set_plugin_catalogue(vec![]);
    assert!(load_plugin(&path, &target, &mut workspace).is_err());
    assert!(workspace.take_simulation_actions().is_empty());
}
