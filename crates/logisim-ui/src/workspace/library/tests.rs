use super::*;
#[test]
fn catalogue_has_unique_stable_groups_and_no_duplicate_tools() {
    let mut ids = std::collections::BTreeSet::new();
    let mut tools = Vec::new();
    for group in groups() {
        assert!(ids.insert(group.key), "Duplicate group ID: {}", group.key);
        assert!(!group.tools.is_empty());
        for &kind in &group.tools {
            assert!(!tools.contains(&kind), "Duplicate library tool: {kind:?}");
            assert!(!matches!(kind, Kind::Subcircuit(_)));
            tools.push(kind);
        }
    }
}

#[test]
fn label_size_defaults_follow_label_support_for_all_builtin_tools() {
    use crate::{Settings, Workspace, editor::Tool};
    use logisim_core::{Component, Point};

    let mut app = Workspace::default();
    for size in [None, Some(24)] {
        app.apply_settings(Settings {
            default_font_size: size,
            ..app.preferences()
        });
        for &kind in groups().iter().flat_map(|group| &group.tools) {
            app.editor.set_tool(Tool::Place(kind));
            let native = Component::new(kind, Point::ZERO).font.size;
            let expected = if kind.has_label() {
                size.unwrap_or(native)
            } else {
                native
            };
            assert_eq!(
                app.editor.template.font.size, expected,
                "{kind:?}, {size:?}"
            );
        }
    }
}
